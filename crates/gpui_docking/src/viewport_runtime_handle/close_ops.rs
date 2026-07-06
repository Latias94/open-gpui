use super::*;

impl DockViewportRuntimeHandle {
    /// Handles a GPUI window-closed notification and applies close policies that mutate graph.
    pub fn handle_window_closed_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        let closed = self
            .runtime
            .borrow_mut()
            .handle_window_closed_with_app_and_refresh(window_id, cx);
        let closed_effects = closed.window_effects();
        let _ = clear_dockhost_drop_previews(closed_effects.refresh().iter().cloned(), cx);
        apply_viewport_window_effects(closed_effects.clone(), cx);
        let _ = self.apply_close_recovery_activation(&closed.outcome, cx);
        closed.outcome
    }

    /// Handles a GPUI window should-close query with workspace lifecycle policy.
    pub fn handle_window_should_close_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseOutcome {
        let should_close = self
            .runtime
            .borrow_mut()
            .handle_window_should_close_with_app_and_refresh(window_id, cx);
        apply_viewport_window_effects(should_close.window_effects(), cx);
        should_close.outcome
    }

    /// Cancels a previously accepted platform close request when the platform did not close.
    ///
    /// The viewport remains registered, but its route facts stay stale until the next host render
    /// publishes fresh platform and host geometry.
    pub fn cancel_window_close_request_with_app(&self, window_id: WindowId, cx: &mut App) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .cancel_window_close_request(window_id);
        refresh_runtime_update(update, cx)
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

    pub(super) fn ensure_window_closed_observer(&self, cx: &mut App) {
        if self.window_closed_observer_installed.replace(true) {
            return;
        }

        let runtime = Rc::downgrade(&self.runtime);
        cx.on_window_closed(move |cx, window_id| {
            let Some(runtime) = runtime.upgrade() else {
                return;
            };
            let closed = runtime
                .borrow_mut()
                .handle_window_closed_with_app_and_refresh(window_id, cx);
            let closed_effects = closed.window_effects();
            let _ = clear_dockhost_drop_previews(closed_effects.refresh().iter().cloned(), cx);
            apply_viewport_window_effects(closed_effects.clone(), cx);
            let _ = apply_close_recovery_activation_for_runtime(&runtime, &closed.outcome, cx);
        })
        .detach();
    }
}
