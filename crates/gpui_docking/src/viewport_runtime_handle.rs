#[cfg(test)]
use crate::DockViewportActivationTransaction;
use crate::{
    DockActionApplyError, DockController, DockDropDelivery, DockHost, DockItemId, DockSpaceId,
    DockViewportCloseOutcome, DockViewportClosePolicy, DockViewportDropRouteOutcome,
    DockViewportDropRouteRequest, DockViewportOpenOutcome, DockViewportOpenStatus,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportResolvedDropRoute, DockViewportRestoreReadiness, DockViewportRoutedDropPreview,
    DockViewportRuntime, DockViewportRuntimeStatus, DockViewportShouldCloseOutcome,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffOpenOutcome, DockViewportTearOffPending, DockViewportTearOffRequest,
    DockViewportWindowFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::DockRuntimeDragSession,
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistration},
    viewport_platform_sync::sync_reused_viewport_window,
    viewport_runtime::{
        DockViewportPointerInputSyncRequest, DockViewportPreparedTearOffDrop,
        DockViewportReusableWindow,
    },
};
#[cfg(test)]
use crate::{
    DockNodeId, DockViewportDropPayload, DockViewportDropRoute, DockViewportPlatformSignals,
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

fn refresh_windows<C: open_gpui::AppContext>(windows: Vec<AnyWindowHandle>, cx: &mut C) {
    for window in windows {
        let _ = window.update(cx, |_, window, _| window.refresh());
    }
}

fn unsupported_pointer_input_sync(
    window_id: WindowId,
    accepts_pointer_input: bool,
) -> crate::DockViewportPlatformSyncRecord {
    crate::DockViewportPlatformSyncRecord {
        window_id,
        applied: Vec::new(),
        skipped_requests: Vec::new(),
        unsupported_requests: vec![crate::DockViewportPlatformSyncUnsupported {
            request: crate::DockViewportPlatformSyncRequest::PointerInput {
                requested: accepts_pointer_input,
            },
            reason: crate::DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi,
        }],
    }
}

fn apply_pointer_input_sync_request(
    sync: Option<DockViewportPointerInputSyncRequest>,
    cx: &mut App,
) -> Option<crate::DockViewportPlatformSyncRecord> {
    let sync = sync?;
    let window = sync.window();
    let accepts_pointer_input = sync.requested_accepts_pointer_input();
    let window_id = window.window_id();
    Some(
        window
            .update(cx, |_, window, _| {
                if window.accepts_pointer_input() == accepts_pointer_input {
                    return crate::DockViewportPlatformSyncRecord {
                        window_id,
                        applied: Vec::new(),
                        skipped_requests: Vec::new(),
                        unsupported_requests: Vec::new(),
                    };
                }
                if window.set_accepts_pointer_input(accepts_pointer_input) {
                    crate::DockViewportPlatformSyncRecord {
                        window_id,
                        applied: vec![crate::DockViewportPlatformSyncAction::PointerInput {
                            enabled: accepts_pointer_input,
                        }],
                        skipped_requests: Vec::new(),
                        unsupported_requests: Vec::new(),
                    }
                } else {
                    unsupported_pointer_input_sync(window_id, accepts_pointer_input)
                }
            })
            .unwrap_or_else(|_| unsupported_pointer_input_sync(window_id, accepts_pointer_input)),
    )
}

pub(crate) fn sync_render_passthrough_pointer_input(
    runtime: &DockViewportRuntimeHandle,
    window: &mut Window,
    passthrough: bool,
) -> bool {
    let window_id = window.window_handle().window_id();
    if passthrough {
        if !window.accepts_pointer_input() {
            return false;
        }
        runtime
            .runtime
            .borrow_mut()
            .record_render_passthrough_pointer_input(window_id);
        return apply_render_pointer_input_sync(runtime, window, false);
    }

    if !runtime
        .runtime
        .borrow_mut()
        .take_render_passthrough_pointer_input(window_id)
    {
        return false;
    }
    if window.accepts_pointer_input() {
        return false;
    }
    apply_render_pointer_input_sync(runtime, window, true)
}

fn apply_render_pointer_input_sync(
    runtime: &DockViewportRuntimeHandle,
    window: &mut Window,
    accepts_pointer_input: bool,
) -> bool {
    let window_id = window.window_handle().window_id();
    let sync_record = if window.set_accepts_pointer_input(accepts_pointer_input) {
        crate::DockViewportPlatformSyncRecord {
            window_id,
            applied: vec![crate::DockViewportPlatformSyncAction::PointerInput {
                enabled: accepts_pointer_input,
            }],
            skipped_requests: Vec::new(),
            unsupported_requests: Vec::new(),
        }
    } else {
        unsupported_pointer_input_sync(window_id, accepts_pointer_input)
    };
    runtime
        .runtime
        .borrow_mut()
        .record_platform_sync(sync_record);
    true
}

fn apply_close_recovery_activation_for_runtime(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    outcome: &DockViewportCloseOutcome,
    cx: &mut App,
) -> DockViewportActivationApplyOutcome {
    let activation = runtime
        .borrow_mut()
        .activation_transaction_after_close(outcome, cx);
    apply_viewport_activation_transaction(activation, cx)
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

fn close_window_quietly(window: AnyWindowHandle, cx: &mut App) {
    let _ = window.update(cx, |_, window, _| window.remove_window());
}

fn close_windows_quietly(windows: Vec<AnyWindowHandle>, cx: &mut App) {
    for window in windows {
        close_window_quietly(window, cx);
    }
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

    pub(crate) fn reconcile_backend_window_focus(&self, cx: &mut App) -> bool {
        self.runtime.borrow_mut().reconcile_backend_window_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<DockViewportActivationTransaction> {
        self.runtime.borrow().pending_activation().cloned()
    }

    pub(crate) fn record_pending_activation(
        &self,
        activation: crate::DockViewportActivationTransaction,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .record_pending_activation(activation)
    }

    pub(crate) fn clear_pending_activation_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .clear_pending_activation_for(space, window_id)
    }

    pub(crate) fn record_panel_focus(&self, space: DockSpaceId, item: DockItemId) {
        self.runtime.borrow_mut().record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&self, space: &DockSpaceId) {
        self.runtime.borrow_mut().record_no_panel_focus(space);
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
        let (changed, windows) = self
            .runtime
            .borrow_mut()
            .mark_viewport_window_snapshot_stale(window_id);
        refresh_windows(windows, cx);
        changed
    }

    pub(crate) fn apply_platform_window_facts(
        &self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        cx: &mut App,
    ) -> bool {
        let (changed, windows) = self
            .runtime
            .borrow_mut()
            .apply_platform_window_facts(window_id, window_facts);
        refresh_windows(windows, cx);
        changed
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
        let (session, pointer_sync) = self
            .runtime
            .borrow_mut()
            .begin_payload_drag_with_pointer_sync_and_focus(payload, focus_item);
        if let Some(sync_record) = apply_pointer_input_sync_request(pointer_sync, cx) {
            self.runtime.borrow_mut().record_platform_sync(sync_record);
        }
        self.reconcile_viewport_frame(cx);
        session
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
        let (changed, windows, pointer_sync) = self
            .runtime
            .borrow_mut()
            .finish_payload_drag_with_pointer_sync(session);
        if let Some(sync_record) = apply_pointer_input_sync_request(pointer_sync, cx) {
            self.runtime.borrow_mut().record_platform_sync(sync_record);
        }
        let reconciled = self.reconcile_viewport_frame(cx);
        refresh_windows(windows, cx);
        changed || reconciled
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
        self.ensure_window_closed_observer(cx);

        let space = space.into();
        let reusable_window = {
            self.runtime
                .borrow_mut()
                .reusable_window_for_space(&space, cx)
        };
        let status = match reusable_window {
            DockViewportReusableWindow::Reused(window) => {
                install_should_close_hook(self.clone(), window, cx)?;
                let platform_requests = self.runtime.borrow().platform_requests_for_space(&space);
                let sync_record = window.update(cx, |_, window, _| {
                    sync_reused_viewport_window(window, options, platform_requests)
                })?;
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

        let replaced_windows = self
            .runtime
            .borrow_mut()
            .register_opened_viewport(space.clone(), window);
        close_windows_quietly(replaced_windows, cx);
        refresh_windows(vec![window], cx);

        Ok(DockViewportOpenOutcome::new(space, window, status))
    }

    fn open_unregistered_viewport_window(
        &self,
        space: DockSpaceId,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<AnyWindowHandle> {
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
        if self.is_viewport_open(&target_space) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("tear-off target space {target_space} is already open"),
            )
            .into());
        }
        {
            let runtime = self.runtime.borrow();
            let controller = runtime.controller_entity();
            let controller = controller.read(cx);
            crate::preflight_tear_off_move(controller.workspace(), &request, &target_space)?;
        }
        let pending = {
            let mut runtime = self.runtime.borrow_mut();
            match runtime.begin_tear_off_request(request, target_space.clone(), cx) {
                DockViewportTearOffBeginOutcome::Pending(pending) => pending,
                DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                    let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                    runtime.record_tear_off_outcome(&outcome);
                    return Ok(outcome);
                }
            }
        };

        self.complete_opened_tear_off_viewport(pending, options, cx)
    }

    fn open_prepared_tear_off_viewport(
        &self,
        prepared: DockViewportPreparedTearOffDrop,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        if self.is_viewport_open(&prepared.target_space) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "tear-off target space {} is already open",
                    prepared.target_space
                ),
            )
            .into());
        }
        let pending = {
            let mut runtime = self.runtime.borrow_mut();
            match runtime.begin_tear_off_request_with_focus(
                prepared.request,
                prepared.target_space.clone(),
                prepared.focus_item,
            ) {
                DockViewportTearOffBeginOutcome::Pending(pending) => pending,
                DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                    let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                    runtime.record_tear_off_outcome(&outcome);
                    return Ok(outcome);
                }
            }
        };

        self.complete_opened_tear_off_viewport(pending, prepared.options, cx)
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
                    "tear-off source no longer available before completion: {:?}",
                    cancelled.reason()
                ),
            )
            .into());
        }

        let action = {
            let mut runtime = self.runtime.borrow_mut();
            match runtime.commit_prepared_tear_off_move(&pending, cx) {
                Ok(action) => action,
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
            let completed = runtime.complete_committed_tear_off_window(pending, action, window);
            let outcome = DockViewportTearOffOpenOutcome::Completed(completed);
            runtime.record_tear_off_outcome(&outcome);
            outcome
        };
        if let DockViewportTearOffOpenOutcome::Completed(completed) = &outcome {
            close_windows_quietly(completed.replaced_windows().to_vec(), cx);
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

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.runtime.borrow_mut().begin_viewport_host_scene(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
        )
    }

    pub(crate) fn unregister_host_for_space_with_app(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        cx: &mut App,
    ) -> bool {
        let (changed, pointer_sync) = self
            .runtime
            .borrow_mut()
            .unregister_host_for_space_with_pointer_sync(space, window_id);
        if let Some(sync_record) = apply_pointer_input_sync_request(pointer_sync, cx) {
            self.runtime.borrow_mut().record_platform_sync(sync_record);
        }
        self.reconcile_viewport_frame(cx);
        changed
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: crate::DockDropGuideStyle,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.runtime.borrow_mut().begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
            drop_guide_style,
        )
    }

    pub(crate) fn register_rendered_host_viewport(
        &self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .register_rendered_host_viewport(space, window)
    }

    pub(crate) fn reconcile_viewport_frame<C: open_gpui::AppContext>(&self, cx: &mut C) -> bool {
        let (changed, windows) = self.runtime.borrow_mut().reconcile_viewport_frame(cx);
        refresh_windows(windows, cx);
        changed
    }

    pub(crate) fn reconcile_viewport_frame_except_window<C: open_gpui::AppContext>(
        &self,
        skip_window_id: WindowId,
        cx: &mut C,
    ) -> bool {
        let (changed, windows) = self
            .runtime
            .borrow_mut()
            .reconcile_viewport_frame_except_window(Some(skip_window_id), cx);
        refresh_windows(windows, cx);
        changed
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_frame_fact(frame, fact)
    }

    pub(crate) fn window_id_for_space(&self, space: &DockSpaceId) -> Option<WindowId> {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .map(|window| window.window_id())
    }

    pub(crate) fn deliver_drop_commit_delivery(
        &self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = match delivery.into_tear_off_request() {
            Ok(request) => self.commit_tear_off_drop_route(request, cx),
            Err(delivery) => self
                .runtime
                .borrow_mut()
                .deliver_drop_commit_delivery_with_outcome(delivery, cx),
        };
        self.clear_routed_drop_preview(cx);
        result
    }

    fn commit_tear_off_drop_route(
        &self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let prepared = {
            let mut runtime = self.runtime.borrow_mut();
            runtime.prepare_tear_off_drop_delivery(request, cx)?
        };

        let result = self
            .open_prepared_tear_off_viewport(prepared, cx)
            .map(DockViewportDropRouteOutcome::tear_off)
            .map_err(|error| DockActionApplyError::TearOffViewportOpenFailed {
                message: error.to_string(),
            });
        self.runtime.borrow_mut().record_drop_route_result(&result);
        result
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.runtime.borrow().last_host_scene_screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        self.runtime
            .borrow()
            .resolve_host_scene_target(space, host_position, cx)
    }

    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_delivery(request, cx)
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_delivery_for_request(request, cx)
    }

    pub(crate) fn update_routed_drop_preview(
        &self,
        resolution: &DockViewportResolvedDropRoute,
        payload_title: &str,
        cx: &mut App,
    ) -> bool {
        let (changed, windows) = self
            .runtime
            .borrow_mut()
            .update_routed_drop_preview(resolution, payload_title);
        refresh_windows(windows, cx);
        changed
    }

    pub(crate) fn clear_routed_drop_preview(&self, cx: &mut App) -> bool {
        let (changed, windows) = self.runtime.borrow_mut().clear_routed_drop_preview();
        refresh_windows(windows, cx);
        changed
    }

    pub(crate) fn finish_routed_drop_acceptance_pass(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .finish_routed_drop_acceptance_pass(space, window_id)
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.runtime
            .borrow()
            .routed_drop_preview_for(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn has_routed_drop_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        self.runtime
            .borrow()
            .has_routed_drop_preview_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn routed_drop_preview_is_accepted(&self) -> bool {
        self.runtime.borrow().routed_drop_preview_is_accepted()
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockViewportIdentity> {
        self.runtime
            .borrow()
            .last_routed_viewport_identity_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> DockViewportDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_route_for_test(request, cx)
    }

    /// Resolves and commits a rendered payload release from a screen-space point.
    pub(crate) fn commit_payload_drop_from_screen(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let resolution = self.resolve_payload_drop_delivery_for_request(request, cx);
        let delivery = match DockDropDelivery::from_resolution(resolution) {
            Ok(delivery) => delivery,
            Err(error) => {
                let result = Err(error);
                self.runtime.borrow_mut().record_drop_route_result(&result);
                return result;
            }
        };
        self.deliver_drop_commit_delivery(delivery, cx)
    }

    /// Resolves and commits a rendered payload release from platform signal snapshots in tests.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_payload_drop_from_screen_with_platform_signals(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals,
            release_origin,
        );
        self.commit_payload_drop_from_screen(&request, cx)
    }

    /// Handles a GPUI window-closed notification and applies close policies that mutate graph.
    pub fn handle_window_closed_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        let outcome = self
            .runtime
            .borrow_mut()
            .handle_window_closed_with_app(window_id, cx);
        let _ = self.apply_close_recovery_activation(&outcome, cx);
        outcome
    }

    /// Handles a GPUI window should-close query with workspace lifecycle policy.
    pub fn handle_window_should_close_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseOutcome {
        let (outcome, windows) = self
            .runtime
            .borrow_mut()
            .handle_window_should_close_with_app_and_refresh(window_id, cx);
        refresh_windows(windows, cx);
        outcome
    }

    /// Cancels a previously accepted platform close request when the platform did not close.
    ///
    /// The viewport remains registered, but its route facts stay stale until the next host render
    /// publishes fresh platform and host geometry.
    pub fn cancel_window_close_request_with_app(&self, window_id: WindowId, cx: &mut App) -> bool {
        let (changed, windows) = self
            .runtime
            .borrow_mut()
            .cancel_window_close_request(window_id);
        refresh_windows(windows, cx);
        changed
    }

    /// Ensures the application-level close observer is installed.
    ///
    /// [`Self::open_viewport`] installs this observer automatically before opening a runtime
    /// viewport. This method remains available for callers that want to eagerly install the same
    /// observer before the first window opens.
    ///
    /// The returned subscription is intentionally inert because observer lifetime is owned by the
    /// runtime handle and the GPUI application callback. Dropping it does not disable cleanup for
    /// runtime-opened windows.
    pub fn observe_window_closed(&self, cx: &mut App) -> Subscription {
        self.ensure_window_closed_observer(cx);
        Subscription::new(|| {})
    }

    fn ensure_window_closed_observer(&self, cx: &mut App) {
        if self.window_closed_observer_installed.replace(true) {
            return;
        }

        let runtime = Rc::downgrade(&self.runtime);
        cx.on_window_closed(move |cx, window_id| {
            let Some(runtime) = runtime.upgrade() else {
                return;
            };
            let outcome = runtime
                .borrow_mut()
                .handle_window_closed_with_app(window_id, cx);
            let _ = apply_close_recovery_activation_for_runtime(&runtime, &outcome, cx);
        })
        .detach();
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
