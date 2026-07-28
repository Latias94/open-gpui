use super::*;

impl DockViewportRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn prepare_and_finalize_window_closed_for_test(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> crate::viewport_window_lifecycle::DockViewportClosedWindowRefresh {
        let prepared = self.runtime.borrow_mut().prepare_window_closed(window_id);
        let applied = prepared.apply_merge_back(cx);
        self.runtime
            .borrow_mut()
            .finalize_window_closed(applied)
            .into_refresh()
    }

    #[cfg(test)]
    pub(crate) fn prepare_and_finalize_close_recovery_for_test(
        &self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> crate::viewport_window_lifecycle::DockViewportCloseRecoveryActivation {
        let prepared = self.runtime.borrow().prepare_close_recovery_window(outcome);
        let applied = prepared.map(|prepared| prepared.sample(cx));
        self.runtime
            .borrow_mut()
            .finalize_close_recovery_activation(outcome, applied)
    }

    /// Handles a GPUI window-closed notification and applies close policies that mutate graph.
    pub fn handle_window_closed_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        self.with_surface_transaction(cx, |surface_transaction, cx| {
            self.clear_platform_mutation_observation_subscriptions(window_id);
            let prepared = self.runtime.borrow_mut().prepare_window_closed(window_id);
            let applied = prepared.apply_merge_back(cx);
            #[cfg(test)]
            self.run_window_close_apply_hook_for_test(cx);
            let finalized = self.runtime.borrow_mut().finalize_window_closed(applied);
            let is_current = finalized.is_current();
            let closed = finalized.into_refresh();
            let mut update = DockViewportRuntimeUpdate::default();
            update.mark_viewport_topology(
                is_current && viewport_close_removed_runtime_mapping(&closed.outcome),
                surface_transaction,
            );
            update.mark_graph_commit(
                closed.outcome.status() == DockViewportCloseStatus::MergedBack,
                surface_transaction,
            );
            self.publish_surface_commit(&update, cx);
            if is_current {
                let closed_effects = closed.window_effects();
                let _ = clear_dockhost_drop_previews(closed_effects.refresh().iter().cloned(), cx);
                apply_viewport_window_effects(&self.runtime, closed_effects.clone(), cx);
                let _ = self.apply_close_recovery_activation(&closed.outcome, cx);
            }
            closed.outcome
        })
    }

    /// Handles a GPUI window should-close query with workspace lifecycle policy.
    pub fn handle_window_should_close_with_app(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseOutcome {
        let prepared = self
            .runtime
            .borrow_mut()
            .prepare_window_should_close(window_id);
        let applied = prepared.apply(cx);
        let finalized = self
            .runtime
            .borrow_mut()
            .finalize_window_should_close(applied);
        let is_current = finalized.is_current();
        let should_close = finalized.into_refresh();
        if is_current {
            apply_viewport_window_effects_excluding(
                &self.runtime,
                should_close.window_effects(),
                Some(window_id),
                cx,
            );
        }
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
        let platform_mutation_observation_subscriptions =
            self.platform_mutation_observation_subscriptions.clone();
        let pending_platform_mutations = self.pending_platform_mutations.clone();
        let terminal_platform_mutations = self.terminal_platform_mutations.clone();
        let open_reservations = self.open_reservations.clone();
        let surface_commit_sink = self.surface_commit_sink.clone();
        let active_surface_transaction = self.active_surface_transaction.clone();
        let surface_owner = self.surface_owner.clone();
        #[cfg(test)]
        let window_close_apply_test_hook = self.window_close_apply_test_hook.clone();
        cx.on_window_closed(move |cx, window_id| {
            let Some(runtime) = runtime.upgrade() else {
                platform_mutation_observation_subscriptions
                    .borrow_mut()
                    .retain(|(observed_window_id, _, _), _| *observed_window_id != window_id);
                pending_platform_mutations
                    .borrow_mut()
                    .retain(|(pending_window_id, _), _| *pending_window_id != window_id);
                terminal_platform_mutations
                    .borrow_mut()
                    .retain(|(terminal_window_id, _), _| *terminal_window_id != window_id);
                return;
            };
            let handle = DockViewportRuntimeHandle {
                runtime,
                window_closed_observer_installed: Rc::new(Cell::new(true)),
                platform_mutation_observation_subscriptions:
                    platform_mutation_observation_subscriptions.clone(),
                pending_platform_mutations: pending_platform_mutations.clone(),
                terminal_platform_mutations: terminal_platform_mutations.clone(),
                open_reservations: open_reservations.clone(),
                surface_commit_sink: surface_commit_sink.clone(),
                active_surface_transaction: active_surface_transaction.clone(),
                surface_owner: surface_owner.clone(),
                #[cfg(test)]
                window_close_apply_test_hook: window_close_apply_test_hook.clone(),
            };
            let _ = handle.handle_window_closed_with_app(window_id, cx);
        })
        .detach();
    }
}
