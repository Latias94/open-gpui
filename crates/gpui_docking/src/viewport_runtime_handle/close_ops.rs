use super::*;

impl DockViewportRuntimeHandle {
    /// Handles a GPUI window-closed notification and applies close policies that mutate graph.
    pub fn handle_window_closed_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        self.with_surface_transaction(cx, |surface_transaction, cx| {
            let closed = self
                .runtime
                .borrow_mut()
                .handle_window_closed_with_app_and_refresh(window_id, cx);
            let mut update = DockViewportRuntimeUpdate::default();
            update.mark_viewport_topology(
                viewport_close_removed_runtime_mapping(&closed.outcome),
                surface_transaction,
            );
            update.mark_graph_commit(
                closed.outcome.status() == DockViewportCloseStatus::MergedBack,
                surface_transaction,
            );
            self.publish_surface_commit(&update, cx);
            let closed_effects = closed.window_effects();
            let _ = clear_dockhost_drop_previews(closed_effects.refresh().iter().cloned(), cx);
            apply_viewport_window_effects(closed_effects.clone(), cx);
            let _ = self.apply_close_recovery_activation(&closed.outcome, cx);
            closed.outcome
        })
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
        let surface_commit_sink = self.surface_commit_sink.clone();
        let active_surface_transaction = self.active_surface_transaction.clone();
        let surface_owner = self.surface_owner.clone();
        cx.on_window_closed(move |cx, window_id| {
            let Some(runtime) = runtime.upgrade() else {
                return;
            };
            let run = |surface_transaction: Option<DockSurfaceTransactionId>, cx: &mut App| {
                let closed = runtime
                    .borrow_mut()
                    .handle_window_closed_with_app_and_refresh(window_id, cx);
                let closed_effects = closed.window_effects();
                let mut runtime_update = DockViewportRuntimeUpdate::default();
                runtime_update.mark_viewport_topology(
                    viewport_close_removed_runtime_mapping(&closed.outcome),
                    surface_transaction,
                );
                runtime_update.mark_graph_commit(
                    closed.outcome.status() == DockViewportCloseStatus::MergedBack,
                    surface_transaction,
                );
                let _ = clear_dockhost_drop_previews(closed_effects.refresh().iter().cloned(), cx);
                apply_viewport_window_effects(closed_effects.clone(), cx);
                let _ = apply_close_recovery_activation_for_runtime(&runtime, &closed.outcome, cx);
                surface_commit_sink.publish(
                    runtime_update.surface_transaction(),
                    runtime_update.change_categories(),
                    cx,
                );
            };
            if let Some(transaction) = active_surface_transaction.get() {
                run(Some(transaction), cx);
            } else if let Some(owner) = surface_owner
                .borrow()
                .as_ref()
                .and_then(WeakEntity::upgrade)
            {
                with_detached_root_transaction(&owner, cx, |transaction, cx| {
                    run(Some(transaction), cx);
                });
            } else {
                run(None, cx);
            }
        })
        .detach();
    }
}
