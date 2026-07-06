#[cfg(test)]
use crate::DockViewportActivationTransaction;
use crate::{
    DockActionApplyError, DockController, DockDropDelivery, DockHost, DockItemId, DockNodeId,
    DockSpaceId, DockViewportCloseOutcome, DockViewportClosePolicy, DockViewportDropRouteOutcome,
    DockViewportDropRouteRequest, DockViewportOpenOutcome, DockViewportOpenStatus,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportPlatformFocusRestoreGate, DockViewportResolvedDropRoute,
    DockViewportResolvedDropRouteOutcome, DockViewportRestoreReadiness,
    DockViewportRoutedDropPreview, DockViewportRuntime, DockViewportRuntimeStatus,
    DockViewportRuntimeUpdate, DockViewportShouldCloseOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffOpenOutcome, DockViewportTearOffPending, DockViewportTearOffRequest,
    DockViewportWindowFacts, DockVisualAffordanceDebugSummary, apply_viewport_window_effects,
    close_window_quietly,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::DockRuntimeDragSession,
    refresh_runtime_update, refresh_viewport_window_effects, refresh_windows,
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistration},
    viewport_platform_sync::{
        record_pointer_input_sync_request as record_pointer_input_sync_request_for_runtime,
        sync_render_passthrough_pointer_input as sync_render_passthrough_pointer_input_for_runtime,
        sync_reused_viewport_window, unavailable_reused_viewport_window_sync,
    },
    viewport_runtime::{DockViewportPreparedTearOffBegin, DockViewportPreparedTearOffDrop},
    viewport_window_lifecycle::DockViewportReusableWindow,
};
#[cfg(test)]
use crate::{
    DockViewportDropPayload, DockViewportDropRoute, DockViewportPlatformSignals,
    interaction::DockPayloadDropReleaseOrigin,
    viewport_registry::DockViewportRouteUnavailableReason,
};
#[cfg(test)]
use open_gpui::WindowBounds;
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Pixels, Point, Result, Subscription,
    Window, WindowId, WindowOptions,
};
#[cfg(test)]
use std::cell::{Ref, RefMut};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

mod close_ops;
mod route_ops;
mod scene_ops;

/// Cloneable application handle for the shared viewport runtime.
///
/// GPUI application-level callbacks such as [`App::on_window_closed`] require `'static` closures.
/// This handle hides the required interior mutability while keeping the runtime itself testable as
/// a normal Rust value.
#[derive(Clone, Debug)]
pub struct DockViewportRuntimeHandle {
    runtime: Rc<RefCell<DockViewportRuntime>>,
    window_closed_observer_installed: Rc<Cell<bool>>,
}

fn clear_dockhost_drop_preview_for_window(window: AnyWindowHandle, cx: &mut App) -> bool {
    window
        .update(cx, |view, _window, cx| {
            let Ok(host) = view.downcast::<DockHost>() else {
                return false;
            };
            host.update(cx, |host, _cx| host.clear_drop_preview_interaction())
        })
        .unwrap_or(false)
}

fn clear_dockhost_drop_previews(
    windows: impl IntoIterator<Item = AnyWindowHandle>,
    cx: &mut App,
) -> bool {
    let mut changed = false;
    let mut cleared_window_ids = Vec::new();
    for window in windows {
        if cleared_window_ids
            .iter()
            .any(|window_id| *window_id == window.window_id())
        {
            continue;
        }
        cleared_window_ids.push(window.window_id());
        changed |= clear_dockhost_drop_preview_for_window(window, cx);
    }
    changed
}

fn apply_pointer_synced_runtime_update<C: open_gpui::AppContext>(
    runtime: &DockViewportRuntimeHandle,
    update: DockViewportRuntimeUpdate,
    cx: &mut C,
) -> bool {
    {
        let mut runtime_core = runtime.runtime.borrow_mut();
        record_pointer_input_sync_request_for_runtime(
            &mut runtime_core,
            update.pointer_input_sync(),
            cx,
        );
    }
    let reconciled = runtime.reconcile_viewport_frame(cx);
    let changed = refresh_runtime_update(update, cx);
    changed || reconciled
}

#[derive(Debug)]
pub(crate) struct DockViewportRenderedHostScenePreparation {
    pub(crate) changed: bool,
    pub(crate) frame: Option<DockViewportHostSceneFrame>,
}

fn apply_close_recovery_activation_for_runtime(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    outcome: &DockViewportCloseOutcome,
    cx: &mut App,
) -> DockViewportActivationApplyOutcome {
    let recovery = runtime
        .borrow_mut()
        .activation_transaction_after_close_with_cleanup(outcome, cx);
    let recovery_effects = recovery.window_effects();
    let _ = clear_dockhost_drop_previews(recovery_effects.refresh().iter().cloned(), cx);
    apply_viewport_window_effects(recovery_effects.clone(), cx);
    apply_viewport_activation_transaction(recovery.activation, cx)
}

fn install_should_close_hook(
    runtime: DockViewportRuntimeHandle,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    window.update(cx, move |_, window, cx| {
        let window_id = window.window_handle().window_id();
        window.on_window_should_close(cx, move |_, cx| {
            runtime
                .handle_window_should_close_with_app(window_id, cx)
                .allows_close()
        });
    })
}

impl DockViewportRuntimeHandle {
    /// Creates a handle around a runtime with the default close policy.
    pub fn new(controller: Entity<DockController>) -> Self {
        DockViewportRuntime::new(controller).into_handle()
    }

    /// Creates a handle around a runtime with an explicit close policy.
    pub fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        DockViewportRuntime::with_close_policy(controller, close_policy).into_handle()
    }

    /// Creates a handle from a prepared runtime.
    pub(crate) fn from_runtime(runtime: DockViewportRuntime) -> Self {
        Self {
            runtime: Rc::new(RefCell::new(runtime)),
            window_closed_observer_installed: Rc::new(Cell::new(false)),
        }
    }

    #[cfg(test)]
    pub(crate) fn borrow(&self) -> Ref<'_, DockViewportRuntime> {
        self.runtime.borrow()
    }

    #[cfg(test)]
    pub(crate) fn borrow_mut(&self) -> RefMut<'_, DockViewportRuntime> {
        self.runtime.borrow_mut()
    }

    /// Returns the shared close policy used by runtime-opened viewport windows.
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.runtime.borrow().close_policy()
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub fn runtime_status(&self) -> DockViewportRuntimeStatus {
        self.runtime.borrow().runtime_status()
    }

    pub(crate) fn record_visual_affordance_status(
        &self,
        space: DockSpaceId,
        window_id: WindowId,
        summary: DockVisualAffordanceDebugSummary,
    ) {
        self.runtime
            .borrow_mut()
            .record_visual_affordance_status(space, window_id, summary);
    }

    pub(crate) fn clear_visual_affordance_status(&self, space: &DockSpaceId, window_id: WindowId) {
        self.runtime
            .borrow_mut()
            .clear_visual_affordance_status(space, window_id);
    }

    #[cfg(test)]
    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        mouse_down: bool,
        cx: &mut App,
    ) -> Option<crate::DockViewportFocusCommand> {
        self.runtime
            .borrow_mut()
            .focus_command_for_confirmed_backend_window_focus(space, window_id, mouse_down, cx)
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut App,
    ) -> crate::DockViewportConfirmedBackendFocusOutcome {
        self.runtime
            .borrow_mut()
            .confirmed_backend_window_focus_outcome(
                space,
                window_id,
                platform_focus_restore_gate,
                cx,
            )
    }

    pub(crate) fn reconcile_backend_window_focus(&self, cx: &mut App) -> bool {
        self.runtime.borrow_mut().reconcile_backend_window_focus(cx)
    }

    pub(crate) fn apply_activation_backend_focus(
        &self,
        activation: &crate::DockViewportActivationTransaction,
        backend_focus: crate::DockViewportActivationBackendFocusObservation,
    ) -> crate::DockViewportActivationBackendFocusApply {
        self.runtime
            .borrow_mut()
            .apply_activation_backend_focus(activation, backend_focus)
    }

    #[cfg(test)]
    pub(crate) fn record_confirmed_backend_focus_for_window(&self, window_id: WindowId) -> bool {
        self.runtime
            .borrow_mut()
            .record_confirmed_backend_focus_for_window(window_id)
    }

    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<DockViewportActivationTransaction> {
        self.runtime.borrow().pending_activation().cloned()
    }

    #[cfg(test)]
    pub(crate) fn record_pending_activation(
        &self,
        activation: crate::DockViewportActivationTransaction,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .record_pending_activation(activation)
    }

    pub(crate) fn record_panel_focus(&self, space: DockSpaceId, item: DockItemId) {
        self.runtime.borrow_mut().record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&self, space: &DockSpaceId) {
        self.runtime.borrow_mut().record_no_panel_focus(space);
    }

    pub(crate) fn recorded_panel_focus_matches(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> bool {
        self.runtime
            .borrow()
            .recorded_panel_focus_matches(space, item)
    }

    pub(crate) fn apply_close_recovery_activation(
        &self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> DockViewportActivationApplyOutcome {
        apply_close_recovery_activation_for_runtime(&self.runtime, outcome, cx)
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus_for_test(&self, space: &DockSpaceId) -> Option<bool> {
        self.runtime
            .borrow()
            .recorded_had_panel_focus_for_test(space)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn mark_viewport_window_snapshot_stale(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .mark_viewport_window_snapshot_stale(window_id);
        refresh_runtime_update(update, cx)
    }

    pub(crate) fn apply_platform_window_facts(
        &self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .apply_platform_window_facts(window_id, window_facts);
        refresh_runtime_update(update, cx)
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag(&self, payload: &DockDragPayload) -> DockRuntimeDragSession {
        self.runtime.borrow_mut().begin_payload_drag(payload)
    }

    pub(crate) fn begin_payload_drag_with_app(
        &self,
        payload: &DockDragPayload,
        cx: &mut App,
    ) -> DockRuntimeDragSession {
        let focus_item = self.runtime.borrow().drag_focus_item(payload, cx);
        let begin = self
            .runtime
            .borrow_mut()
            .begin_payload_drag_with_pointer_sync_and_focus(payload, focus_item);
        record_pointer_input_sync_request_for_runtime(
            &mut self.runtime.borrow_mut(),
            begin.pointer_input_sync,
            cx,
        );
        self.reconcile_viewport_frame(cx);
        begin.session
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .update_payload_drag_tear_off_geometry(session, geometry)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.runtime.borrow().active_payload_drag_session(payload)
    }

    pub(crate) fn record_payload_drag_hovered_viewport(
        &self,
        session: &DockRuntimeDragSession,
        space: DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .record_payload_drag_hovered_viewport(session, space, window_id)
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        self.runtime
            .borrow()
            .active_payload_drag_tear_off_geometry(session)
    }

    pub(crate) fn finish_payload_drag_with_app(
        &self,
        session: &DockRuntimeDragSession,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .finish_payload_drag_with_pointer_sync(session);
        apply_pointer_synced_runtime_update(self, update, cx)
    }

    /// Returns registered dock spaces in stable lexical order.
    pub fn registered_viewport_spaces(&self) -> Vec<DockSpaceId> {
        self.runtime.borrow().adapter().spaces()
    }

    /// Returns true when a logical dock space currently has a runtime window mapping.
    pub fn is_viewport_open(&self, space: &DockSpaceId) -> bool {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .is_some()
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_ready(&self, space: &DockSpaceId) -> bool {
        self.runtime.borrow().viewport_route_ready(space)
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_unavailable_reason(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRouteUnavailableReason> {
        self.runtime
            .borrow()
            .viewport_route_unavailable_reason(space)
    }

    /// Replaces the shared close policy used by runtime-opened viewport windows.
    pub fn set_close_policy(&self, close_policy: DockViewportClosePolicy) {
        self.runtime.borrow_mut().set_close_policy(close_policy);
    }

    fn ensure_platform_viewport_windows_supported(&self, cx: &App) -> Result<()> {
        if cx.viewport_capabilities().platform_viewport_windows {
            return Ok(());
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "platform viewport windows are not supported by this backend",
        )
        .into())
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// The handle installs a should-close hook that consults the shared runtime at close time, so
    /// later close-policy changes are observed by already-open windows.
    pub fn open_viewport(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.ensure_platform_viewport_windows_supported(cx)?;
        self.ensure_window_closed_observer(cx);

        let space = space.into();
        let (reusable, reusable_effects) = {
            self.runtime
                .borrow_mut()
                .reusable_window_for_space_with_cleanup(&space, cx)
                .into_parts()
        };
        let status = match reusable {
            DockViewportReusableWindow::Reused(window) => {
                if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
                    self.runtime.borrow_mut().record_platform_sync(
                        unavailable_reused_viewport_window_sync(window.window_id()),
                    );
                    return Err(error);
                }
                let platform_requests = self.runtime.borrow().platform_requests_for_space(&space);
                let sync_record = match window.update(cx, |_, window, cx| {
                    sync_reused_viewport_window(
                        window,
                        options,
                        platform_requests,
                        cx.viewport_capabilities(),
                        cx.viewport_flag_capabilities(),
                    )
                }) {
                    Ok(sync_record) => sync_record,
                    Err(error) => {
                        self.runtime.borrow_mut().record_platform_sync(
                            unavailable_reused_viewport_window_sync(window.window_id()),
                        );
                        return Err(error);
                    }
                };
                self.runtime.borrow_mut().record_platform_sync(sync_record);
                self.reconcile_viewport_frame(cx);
                refresh_windows(vec![window], cx);
                return Ok(DockViewportOpenOutcome::new(
                    space,
                    window,
                    DockViewportOpenStatus::Reused,
                ));
            }
            DockViewportReusableWindow::Stale => DockViewportOpenStatus::Replaced,
            DockViewportReusableWindow::Missing => DockViewportOpenStatus::Opened,
        };
        apply_viewport_window_effects(reusable_effects, cx);

        let controller = self.runtime.borrow().controller_entity();
        let host_space = space.clone();
        let host_runtime = self.clone();
        let window = cx
            .open_window(options, move |_, cx| {
                cx.new(move |cx| {
                    DockHost::from_controller(controller, host_space, host_runtime, cx)
                })
            })?
            .into();

        if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
            close_window_quietly(window, cx);
            return Err(error);
        }

        let registration = self
            .runtime
            .borrow_mut()
            .register_opened_viewport_with_cleanup(space.clone(), window);
        apply_viewport_window_effects(registration.window_effects(), cx);
        refresh_windows(vec![window], cx);

        Ok(DockViewportOpenOutcome::new(space, window, status))
    }

    fn open_unregistered_viewport_window(
        &self,
        space: DockSpaceId,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<AnyWindowHandle> {
        self.ensure_platform_viewport_windows_supported(cx)?;
        self.ensure_window_closed_observer(cx);

        let controller = self.runtime.borrow().controller_entity();
        let host_runtime = self.clone();
        let window = cx
            .open_window(options, move |_, cx| {
                cx.new(move |cx| DockHost::from_controller(controller, space, host_runtime, cx))
            })?
            .into();

        if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
            close_window_quietly(window, cx);
            return Err(error);
        }

        Ok(window)
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    #[cfg(test)]
    pub(crate) fn open_tear_off_viewport(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let target_space = target_space.into();
        let prepared = self
            .runtime
            .borrow_mut()
            .prepare_tear_off_drop_route_for_test(request, target_space, options, cx)?;
        self.open_prepared_tear_off_viewport(prepared, cx)
    }

    fn open_prepared_tear_off_viewport(
        &self,
        prepared: DockViewportPreparedTearOffDrop,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        if self.is_viewport_open(prepared.target_space()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "tear-off target space {} is already open",
                    prepared.target_space()
                ),
            )
            .into());
        }
        let begin = self
            .runtime
            .borrow_mut()
            .begin_prepared_tear_off_drop(prepared);
        match begin {
            DockViewportPreparedTearOffBegin::Pending(prepared_window) => self
                .complete_opened_tear_off_viewport(
                    prepared_window.pending,
                    prepared_window.options,
                    cx,
                ),
            DockViewportPreparedTearOffBegin::Duplicate(pending) => {
                let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                self.runtime.borrow_mut().record_tear_off_outcome(&outcome);
                Ok(outcome)
            }
        }
    }

    fn complete_opened_tear_off_viewport(
        &self,
        pending: DockViewportTearOffPending,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let window = match self.open_unregistered_viewport_window(
            pending.target_space().clone(),
            options,
            cx,
        ) {
            Ok(window) => window,
            Err(error) => {
                self.runtime.borrow_mut().cancel_tear_off_request(
                    &pending.request().key(),
                    DockViewportTearOffCancelReason::Cancelled,
                );
                return Err(error);
            }
        };

        self.finish_opened_tear_off_viewport(pending, window, cx)
    }

    fn finish_opened_tear_off_viewport(
        &self,
        pending: DockViewportTearOffPending,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let key = pending.request().key();
        if let Some(cancelled) = self
            .runtime
            .borrow_mut()
            .cancel_tear_off_if_source_unavailable(&pending, &key, cx)
        {
            close_window_quietly(window, cx);
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "tear-off source no longer available before completion for target {}: {:?}",
                    cancelled.pending().target_space(),
                    cancelled.reason(),
                ),
            )
            .into());
        }

        let committed = {
            let mut runtime = self.runtime.borrow_mut();
            match runtime.commit_prepared_tear_off_move(&pending, cx) {
                Ok(committed) => committed,
                Err(error) => {
                    runtime
                        .cancel_tear_off_request(&key, DockViewportTearOffCancelReason::Cancelled);
                    close_window_quietly(window, cx);
                    return Err(error.into());
                }
            }
        };
        let outcome = {
            let mut runtime = self.runtime.borrow_mut();
            let completed = runtime.complete_committed_tear_off_window(committed, window, cx);
            let outcome = DockViewportTearOffOpenOutcome::Completed(completed);
            runtime.record_tear_off_outcome(&outcome);
            outcome
        };
        if let DockViewportTearOffOpenOutcome::Completed(completed) = &outcome {
            apply_viewport_window_effects(completed.window_effects(), cx);
        }
        Ok(outcome)
    }

    #[cfg(test)]
    pub(crate) fn complete_opened_tear_off_viewport_for_test(
        &self,
        pending: DockViewportTearOffPending,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        self.finish_opened_tear_off_viewport(pending, window, cx)
    }

    /// Exports serializable placement snapshots from the shared runtime.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.runtime.borrow().export_placement()
    }

    /// Checks saved placement snapshots against windows currently registered in the runtime.
    ///
    /// This does not open, move, or resize platform windows. Use
    /// [`DockViewportPlacementLayout::window_options_for_space`] when opening a viewport from
    /// saved placement.
    pub fn check_placement_restore(
        &self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        self.runtime.borrow_mut().check_placement_restore(placement)
    }
}
