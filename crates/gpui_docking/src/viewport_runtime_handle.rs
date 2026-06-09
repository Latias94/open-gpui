use crate::{
    DockController, DockItemId, DockSpaceId, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportOpenOutcome, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreOutcome, DockViewportRuntime, DockViewportShouldCloseOutcome,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason, DockViewportTearOffCancelled,
    DockViewportTearOffCompletionOutcome, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockViewportTearOffTick,
};
use open_gpui::{AnyWindowHandle, App, Entity, Result, Subscription, WindowId, WindowOptions};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

/// Cloneable application handle for a shared [`DockViewportRuntime`].
///
/// GPUI application-level callbacks such as [`App::on_window_closed`] require `'static` closures.
/// This handle hides the required interior mutability while keeping the runtime itself testable as
/// a normal Rust value.
#[derive(Clone)]
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
    pub fn from_runtime(runtime: DockViewportRuntime) -> Self {
        Self {
            runtime: Rc::new(RefCell::new(runtime)),
        }
    }

    /// Borrows the shared runtime.
    pub fn borrow(&self) -> Ref<'_, DockViewportRuntime> {
        self.runtime.borrow()
    }

    /// Returns the shared close policy used by runtime-opened viewport windows.
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.runtime.borrow().close_policy()
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
        let runtime = self.clone();
        self.runtime.borrow_mut().open_viewport_with_should_close(
            space,
            options,
            cx,
            move |window_id| runtime.handle_window_should_close(window_id).allows_close(),
        )
    }

    /// Returns pending tear-off item ids in stable order.
    pub fn pending_tear_off_items(&self) -> Vec<DockItemId> {
        self.runtime.borrow().pending_tear_off_items()
    }

    /// Returns the number of pending tear-off transactions.
    pub fn pending_tear_off_len(&self) -> usize {
        self.runtime.borrow().pending_tear_off_len()
    }

    /// Returns the pending tear-off request for an item.
    pub fn pending_tear_off(&self, item: &DockItemId) -> Option<DockViewportTearOffPending> {
        self.runtime.borrow().pending_tear_off(item).cloned()
    }

    /// Records a tear-off request without opening a window yet.
    pub fn begin_tear_off_request(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
    ) -> DockViewportTearOffBeginOutcome {
        self.runtime
            .borrow_mut()
            .begin_tear_off_request(request, target_space)
    }

    /// Records a tear-off request at an explicit logical clock value.
    pub fn begin_tear_off_request_at(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.runtime
            .borrow_mut()
            .begin_tear_off_request_at(request, target_space, now)
    }

    /// Cancels a pending tear-off request for an item.
    pub fn cancel_tear_off_request(
        &self,
        item: &DockItemId,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.runtime
            .borrow_mut()
            .cancel_tear_off_request(item, reason)
    }

    /// Removes stale pending tear-off requests at an explicit logical clock value.
    pub fn expire_tear_off_requests_at(
        &self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        self.runtime.borrow_mut().expire_tear_off_requests_at(now)
    }

    /// Completes a pending tear-off request after a platform viewport window exists.
    pub fn complete_tear_off_viewport(
        &self,
        item: &DockItemId,
        window: impl Into<AnyWindowHandle>,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        self.runtime
            .borrow_mut()
            .complete_tear_off_viewport(item, window, cx)
    }

    /// Completes a pending tear-off request at an explicit logical clock value.
    pub fn complete_tear_off_viewport_at(
        &self,
        item: &DockItemId,
        window: impl Into<AnyWindowHandle>,
        now: DockViewportTearOffTick,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        self.runtime
            .borrow_mut()
            .complete_tear_off_viewport_at(item, window, now, cx)
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    pub fn open_tear_off_viewport(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        self.runtime
            .borrow_mut()
            .open_tear_off_viewport(request, target_space, options, cx)
    }

    /// Handles a GPUI window-closed notification through the shared runtime.
    pub fn handle_window_closed(&self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.runtime.borrow_mut().handle_window_closed(window_id)
    }

    /// Handles a GPUI window should-close query through the shared runtime.
    pub fn handle_window_should_close(
        &self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        self.runtime.borrow().handle_window_should_close(window_id)
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
        cx.on_window_closed(move |_, window_id| {
            runtime.handle_window_closed(window_id);
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
