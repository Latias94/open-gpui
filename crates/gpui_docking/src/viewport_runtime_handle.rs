use crate::{
    DockActionApplyError, DockController, DockDropDelivery, DockHost, DockSpaceId,
    DockViewportActivationTarget, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportOpenOutcome,
    DockViewportOpenStatus, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportResolvedDropRoute, DockViewportRestoreReadiness, DockViewportRoutedDropPreview,
    DockViewportRuntime, DockViewportRuntimeStatus, DockViewportShouldCloseOutcome,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffOpenOutcome, DockViewportTearOffRequest, DockViewportWindowFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::DockRuntimeDragSession,
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistration},
    viewport_runtime::{DockViewportReusableWindow, DockViewportTearOffCommitPreparation},
};
#[cfg(test)]
use crate::{
    DockNodeId, DockViewportDropPayload, DockViewportDropRoute, DockViewportPlatformSignals,
    DockViewportTargetContext, viewport_registry::DockViewportRouteUnavailableReason,
};
#[cfg(test)]
use open_gpui::WindowBounds;
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Pixels, Point, Result, Subscription,
    WindowId, WindowOptions,
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

fn refresh_windows(windows: Vec<AnyWindowHandle>, cx: &mut App) {
    for window in windows {
        let _ = window.update(cx, |_, window, _| window.refresh());
    }
}

fn apply_viewport_activation(activation: Option<DockViewportActivationTarget>, cx: &mut App) {
    let Some(activation) = activation else {
        return;
    };
    let activation_space = activation.space().clone();
    let focus_item = activation.focus_item().cloned();
    let _ = activation.window().update(cx, move |view, window, cx| {
        window.activate_window();
        if let Some(focus_item) = focus_item
            && let Ok(host) = view.downcast::<DockHost>()
        {
            host.update(cx, |host, cx| {
                if host.space() == &activation_space && host.request_panel_focus(focus_item) {
                    cx.notify();
                }
            });
        }
    });
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

    pub(crate) fn record_window_focus(&self, window_id: WindowId) {
        self.runtime.borrow_mut().record_window_focus(window_id);
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

    pub(crate) fn begin_payload_drag(&self, payload: &DockDragPayload) -> DockRuntimeDragSession {
        self.runtime.borrow_mut().begin_payload_drag(payload)
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

    #[cfg(test)]
    pub(crate) fn finish_payload_drag(&self, session: &DockRuntimeDragSession) -> bool {
        self.runtime.borrow_mut().finish_payload_drag(session).0
    }

    pub(crate) fn finish_payload_drag_with_app(
        &self,
        session: &DockRuntimeDragSession,
        cx: &mut App,
    ) -> bool {
        let (changed, windows) = self.runtime.borrow_mut().finish_payload_drag(session);
        refresh_windows(windows, cx);
        changed
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
        let status = match self
            .runtime
            .borrow_mut()
            .reusable_window_for_space(&space, cx)
        {
            DockViewportReusableWindow::Reused(window) => {
                install_should_close_hook(self.clone(), window, cx)?;
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
                    DockHost::with_viewport_runtime(controller, host_space, host_runtime, cx)
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

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    pub(crate) fn open_tear_off_viewport(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let key = request.key();
        let pending = {
            let mut runtime = self.runtime.borrow_mut();
            match runtime.begin_tear_off_request(request, target_space, cx) {
                DockViewportTearOffBeginOutcome::Pending(pending) => pending,
                DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                    let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                    runtime.record_tear_off_outcome(&outcome);
                    return Ok(outcome);
                }
            }
        };

        if self.is_viewport_open(pending.target_space()) {
            self.runtime
                .borrow_mut()
                .cancel_tear_off_request(&key, DockViewportTearOffCancelReason::Cancelled);
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "tear-off target space {} is already open",
                    pending.target_space()
                ),
            )
            .into());
        }

        let opened = match self.open_viewport(pending.target_space().clone(), options, cx) {
            Ok(opened) => opened,
            Err(error) => {
                self.runtime
                    .borrow_mut()
                    .cancel_tear_off_request(&key, DockViewportTearOffCancelReason::Cancelled);
                return Err(error);
            }
        };

        let outcome = {
            let mut runtime = self.runtime.borrow_mut();
            let completion = runtime.complete_tear_off_viewport(&key, opened.window(), cx);
            let outcome = runtime.finish_tear_off_open(pending, completion, cx);
            runtime.record_tear_off_outcome(&outcome);
            outcome
        };
        if let DockViewportTearOffOpenOutcome::Completed(completed) = &outcome {
            close_windows_quietly(completed.replaced_windows().to_vec(), cx);
        }
        if !matches!(outcome, DockViewportTearOffOpenOutcome::Completed(_)) {
            close_window_quietly(opened.window(), cx);
        }
        Ok(outcome)
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

    pub(crate) fn begin_viewport_host_scene_frame(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.runtime.borrow_mut().begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
        )
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

    pub(crate) fn deliver_payload_drop_with_outcome(
        &self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = match delivery {
            DockDropDelivery::TearOff(request) => self.commit_tear_off_drop_route(request, cx),
            delivery => self
                .runtime
                .borrow_mut()
                .deliver_payload_drop_with_outcome(delivery, cx),
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
            match runtime.prepare_tear_off_drop_delivery(request, cx)? {
                DockViewportTearOffCommitPreparation::Prepared(prepared) => *prepared,
            }
        };

        let result = self
            .open_tear_off_viewport(
                prepared.request,
                prepared.target_space,
                prepared.options,
                cx,
            )
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

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_route(request, cx)
    }

    pub(crate) fn resolve_payload_drop_delivery(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportResolvedDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_delivery(request, cx)
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
    pub(crate) fn routed_drop_delivery_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDropDelivery> {
        self.runtime
            .borrow()
            .routed_drop_delivery_for_drag_session(session)
    }

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_payload_drop_route_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: DockViewportTargetContext,
        cx: &App,
    ) -> DockViewportDropRoute {
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            target_context,
        );
        self.resolve_payload_drop_route(&request, cx)
    }

    /// Resolves a rendered payload release from platform signal snapshots in tests.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_payload_drop_route_with_platform_signals(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        cx: &App,
    ) -> DockViewportDropRoute {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals,
        );
        self.resolve_payload_drop_route(&request, cx)
    }

    /// Resolves and commits a rendered payload release from a screen-space point.
    pub(crate) fn commit_payload_drop_from_screen(
        &self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let resolution = self.resolve_payload_drop_delivery(request, cx);
        let delivery = resolution.delivery().clone();
        self.deliver_payload_drop_with_outcome(delivery, cx)
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
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals,
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
        let activation = self
            .runtime
            .borrow_mut()
            .activation_target_after_close(&outcome, cx);
        apply_viewport_activation(activation, cx);
        outcome
    }

    /// Handles a GPUI window should-close query through the shared runtime.
    pub fn handle_window_should_close(
        &self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        self.runtime
            .borrow_mut()
            .handle_window_should_close(window_id)
    }

    /// Handles a GPUI window should-close query with workspace lifecycle policy.
    pub fn handle_window_should_close_with_app(
        &self,
        window_id: WindowId,
        cx: &App,
    ) -> DockViewportShouldCloseOutcome {
        self.runtime
            .borrow_mut()
            .handle_window_should_close_with_app(window_id, cx)
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
            let activation = runtime
                .borrow_mut()
                .activation_target_after_close(&outcome, cx);
            apply_viewport_activation(activation, cx);
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
