use crate::{
    DockActionApplyError, DockController, DockHost, DockNodeId, DockSpaceId,
    DockViewportCloseOutcome, DockViewportClosePolicy, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportOpenOutcome,
    DockViewportOpenStatus, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreOutcome, DockViewportRuntime, DockViewportRuntimeStatus,
    DockViewportShouldCloseOutcome, DockViewportTargetContext, DockViewportTearOffBeginOutcome,
    DockViewportTearOffCancelReason, DockViewportTearOffOpenOutcome, DockViewportTearOffRequest,
    drop_runtime::DockHostDropSceneFact, viewport_runtime::DockViewportReusableWindow,
};
use open_gpui::{
    App, AppContext as _, Bounds, Entity, Pixels, Point, Result, Subscription, WindowBounds,
    WindowId, WindowOptions,
};
#[cfg(test)]
use std::cell::Ref;
use std::{cell::RefCell, rc::Rc};

/// Cloneable application handle for the shared viewport runtime.
///
/// GPUI application-level callbacks such as [`App::on_window_closed`] require `'static` closures.
/// This handle hides the required interior mutability while keeping the runtime itself testable as
/// a normal Rust value.
#[derive(Clone, Debug)]
pub struct DockViewportRuntimeHandle {
    runtime: Rc<RefCell<DockViewportRuntime>>,
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
        }
    }

    #[cfg(test)]
    pub(crate) fn borrow(&self) -> Ref<'_, DockViewportRuntime> {
        self.runtime.borrow()
    }

    /// Returns the shared close policy used by runtime-opened viewport windows.
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.runtime.borrow().close_policy()
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub fn runtime_status(&self) -> DockViewportRuntimeStatus {
        self.runtime.borrow().runtime_status()
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
        let space = space.into();
        let status = match self
            .runtime
            .borrow_mut()
            .reusable_window_for_space(&space, cx)
        {
            DockViewportReusableWindow::Reused(window) => {
                return Ok(DockViewportOpenOutcome {
                    space,
                    window,
                    status: DockViewportOpenStatus::Reused,
                });
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

        self.runtime
            .borrow_mut()
            .register_opened_viewport(space.clone(), window);

        let close_runtime = self.clone();
        if let Err(error) = window.update(cx, move |_, window, cx| {
            let window_id = window.window_handle().window_id();
            window.on_window_should_close(cx, move |_, _| {
                close_runtime
                    .handle_window_should_close(window_id)
                    .allows_close()
            });
        }) {
            self.runtime
                .borrow_mut()
                .discard_failed_opened_viewport(window.window_id());
            return Err(error);
        }

        Ok(DockViewportOpenOutcome {
            space,
            window,
            status,
        })
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    pub(crate) fn open_tear_off_viewport(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let key = request
            .payload
            .key(&request.source_space, request.source_tabs);
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

        let opened = match self.open_viewport(pending.target_space.clone(), options, cx) {
            Ok(opened) => opened,
            Err(error) => {
                self.runtime
                    .borrow_mut()
                    .cancel_tear_off_request(&key, DockViewportTearOffCancelReason::Cancelled);
                return Err(error);
            }
        };

        let mut runtime = self.runtime.borrow_mut();
        let completion = runtime.complete_tear_off_viewport(&key, opened.window, cx);
        let outcome = runtime.finish_tear_off_open(pending, completion, cx);
        runtime.record_tear_off_outcome(&outcome);
        Ok(outcome)
    }

    pub(crate) fn begin_viewport_host_scene(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.runtime.borrow_mut().begin_viewport_host_scene(
            space,
            window_id,
            window_bounds,
            host_bounds,
            host_position,
        )
    }

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

    pub(crate) fn window_id_for_space(&self, space: &DockSpaceId) -> Option<WindowId> {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .map(|window| window.window_id())
    }

    pub(crate) fn commit_payload_drop_route_with_outcome(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        route: DockViewportDropRoute,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        if let DockViewportDropRoute::TearOff(request) = route {
            return self.commit_tear_off_drop_route(
                source_space,
                source_tabs,
                payload,
                request,
                cx,
            );
        }

        self.runtime
            .borrow_mut()
            .commit_payload_drop_route_with_outcome(source_space, source_tabs, payload, route, cx)
    }

    fn commit_tear_off_drop_route(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let prepared = {
            let mut runtime = self.runtime.borrow_mut();
            if let Some(outcome) = runtime.single_viewport_outside_release_noop(
                source_space,
                source_tabs,
                &payload,
                cx,
            ) {
                let result = Ok(outcome);
                runtime.record_drop_route_result(&result);
                return result;
            }
            runtime.prepare_tear_off_drop_route(source_space, source_tabs, payload, request, cx)?
        };

        let result = self
            .open_tear_off_viewport(
                prepared.request,
                prepared.target_space,
                prepared.options,
                cx,
            )
            .map(DockViewportDropRouteOutcome::TearOff)
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

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_payload_drop_route_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: &DockViewportTargetContext,
        cx: &App,
    ) -> DockViewportDropRoute {
        self.runtime
            .borrow_mut()
            .resolve_payload_drop_route_with_context(
                source_space,
                source_tabs,
                payload,
                release_position,
                suggested_window_bounds,
                target_context,
                cx,
            )
    }

    /// Resolves and commits a rendered payload release from a screen-space point.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_payload_drop_from_screen_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: &DockViewportTargetContext,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let source_space = source_space.into();
        let route = self.resolve_payload_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            payload.clone(),
            release_position,
            suggested_window_bounds,
            target_context,
            cx,
        );
        self.commit_payload_drop_route_with_outcome(&source_space, source_tabs, payload, route, cx)
    }

    /// Handles a GPUI window-closed notification and applies close policies that mutate graph.
    pub fn handle_window_closed_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        self.runtime
            .borrow_mut()
            .handle_window_closed_with_app(window_id, cx)
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

    /// Registers an application-level close observer that cleans up viewport mappings by
    /// [`WindowId`].
    ///
    /// This observer runs after a close has been accepted. It complements the should-close hook
    /// installed by [`Self::open_viewport`].
    ///
    /// Keep or detach the returned subscription according to the application's lifetime policy.
    pub fn observe_window_closed(&self, cx: &mut App) -> Subscription {
        let runtime = self.clone();
        cx.on_window_closed(move |cx, window_id| {
            runtime.handle_window_closed_with_app(window_id, cx);
        })
    }

    /// Exports serializable placement snapshots from the shared runtime.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.runtime.borrow().export_placement()
    }

    /// Applies saved placement snapshots through the shared runtime.
    pub fn apply_placement(
        &self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        self.runtime.borrow_mut().apply_placement(placement)
    }
}
