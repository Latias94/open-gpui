//! Public runtime entry points and callback dispatch.

use super::*;

impl WindowOverlayRuntime {
    /// Returns the unique overlay runtime owned by the current window.
    pub fn for_window(window: &mut Window, cx: &mut App) -> Self {
        let window_id = window.window_handle().window_id();
        let state =
            window.use_window_state(cx, |window, cx| WindowOverlayRuntimeState::new(window, cx));
        let ambient_parent_layers = state.read(cx).ambient_parent_layers.clone();
        let runtime = Self {
            state,
            window_id,
            ambient_parent_layers,
        };
        runtime.install_interceptors(window, cx);
        runtime
    }

    /// Returns the underlying state identity for isolation assertions.
    pub fn entity_id(&self) -> EntityId {
        self.state.entity_id()
    }

    /// Registers a new stable layer and returns its ownership binding.
    ///
    /// This is the low-level manual-lifetime API. Retain the binding and either call
    /// [`Self::bind_layer_to_entity_release`] or explicitly unregister it. Component code should
    /// normally use [`Self::register_layer_for_entity`].
    pub fn register_layer(
        &self,
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        validate_registration(&registration)?;
        let trigger_focus = cx.focus_handle();
        let surface_focus = cx.focus_handle();
        let (lease, focus_config, activate) = self.state.update(cx, |state, _| {
            state.register_layer(registration, &trigger_focus, &surface_focus, self.window_id)
        })?;

        if let Err(error) = self.install_focus(&focus_config, activate, window, cx) {
            self.state.update(cx, |state, _| {
                state.remove_layer_without_focus(lease.layer_id(), lease.token);
            });
            return Err(error);
        }

        let weak_state = self.state.downgrade();
        let focus_layer_id = lease.layer_id.clone();
        let focus_lease = lease.token;
        let focus_subscription = window.on_focus_in(&surface_focus, cx, move |_, cx| {
            let _ = weak_state.update(cx, |state, _| {
                if state
                    .entries
                    .get(&focus_layer_id)
                    .is_some_and(|entry| entry.lease_token == focus_lease)
                    && let Some(entry) = state.entries.get_mut(&focus_layer_id)
                {
                    entry.focus_entered = true;
                }
            });
        });
        self.state.update(cx, |state, _| {
            if let Some(entry) = state.entries.get_mut(lease.layer_id()) {
                entry.focus_subscription = Some(focus_subscription);
            }
        });

        Ok(OverlayLayerBinding {
            lease,
            trigger_focus,
            surface_focus,
        })
    }

    /// Atomically registers a layer and binds its cleanup to an owner entity.
    pub fn register_layer_for_entity<T: 'static>(
        &self,
        registration: OverlayLayerRegistration,
        owner: &Entity<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        let binding = self.register_layer(registration, window, cx)?;
        if let Err(error) = self.bind_layer_to_entity_release(&binding, owner, window, cx) {
            let _ = self.unregister_layer(&binding, window, cx);
            return Err(error);
        }
        Ok(binding)
    }

    /// Rebinds an existing layer with its latest policy, parent, and callbacks.
    pub fn rebind_layer(
        &self,
        binding: &OverlayLayerBinding,
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        if registration.id != binding.lease.layer_id {
            return Err(WindowOverlayRuntimeError::ForeignLease(registration.id));
        }
        let focus_config = self.state.read(cx).prepare_rebind(
            &binding.lease,
            &registration,
            &binding.trigger_focus,
            &binding.surface_focus,
        )?;
        validate_registration(&registration)?;
        self.rebind_focus(&focus_config, window, cx)?;
        let plan = self.state.update(cx, |state, _| {
            state.rebind_layer_plan(&binding.lease, registration)
        })?;
        self.cancel_focus_claims(&plan.cancel_focus_claims, window, cx)?;
        for dispatch in &plan.descendant_dispatches {
            self.apply_focus_transition(dispatch.focus_transition.clone(), window, cx)?;
        }
        self.apply_focus_transition(plan.root_transition.clone(), window, cx)?;
        self.run_open_change_dispatches(plan.descendant_dispatches, false, window, cx);
        Ok(plan.generation)
    }

    /// Binds the layer registration lifetime to a keyed owner entity.
    ///
    /// The release callback only schedules cleanup. Unregistration begins after the release
    /// transaction has completed, so focus restoration never mutates the releasing entity.
    pub fn bind_layer_to_entity_release<T: 'static>(
        &self,
        binding: &OverlayLayerBinding,
        owner: &Entity<T>,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state.read(cx).validate_lease(&binding.lease)?;

        let weak_state = self.state.downgrade();
        let window_id = self.window_id;
        let lease = binding.lease.clone();
        let subscription = cx.observe_release_in(owner, window, move |_, window, cx| {
            let weak_state = weak_state.clone();
            let lease = lease.clone();
            window.defer(cx, move |window, cx| {
                let Some(state) = weak_state.upgrade() else {
                    return;
                };
                let ambient_parent_layers = state.read(cx).ambient_parent_layers.clone();
                let runtime = WindowOverlayRuntime {
                    state,
                    window_id,
                    ambient_parent_layers,
                };
                let _ = runtime.unregister_released_subtree_by_lease(lease, window, cx);
            });
        });
        self.state.update(cx, |state, _| {
            let entry = state.entry_for_lease_mut(&binding.lease)?;
            entry.release_subscription = Some(subscription);
            Ok(())
        })
    }

    /// Requests an open-state change through the registered owner semantics.
    pub fn request_open_change(
        &self,
        binding: &OverlayLayerBinding,
        open: bool,
        reason: DismissReason,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.request_open_change_by_id(
            binding.lease.layer_id.clone(),
            Some(binding.lease.token),
            open,
            reason,
            window,
            cx,
        )
    }

    /// Rejects the matching controlled close intent while preserving open authority.
    pub fn reject_controlled_intent(
        &self,
        binding: &OverlayLayerBinding,
        revision: OverlayOpenIntentRevision,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state.update(cx, |state, _| {
            state.reject_controlled_intent(&binding.lease, revision)
        })?;
        window.refresh();
        Ok(())
    }

    /// Completes exit presence for the matching closing generation.
    pub fn finish_exit(
        &self,
        binding: &OverlayLayerBinding,
        generation: OverlayLayerGeneration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state
            .update(cx, |state, _| state.finish_exit(&binding.lease, generation))
    }

    /// Unregisters a layer after any queued focus restoration has committed.
    pub fn unregister_layer(
        &self,
        binding: &OverlayLayerBinding,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.unregister_layer_by_lease(binding.lease.clone(), window, cx)
    }

    pub(super) fn unregister_layer_by_lease(
        &self,
        lease: OverlayLayerLease,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let transition = self
            .state
            .update(cx, |state, _| state.begin_unregister(&lease))?;
        self.apply_focus_transition(transition, window, cx)?;
        self.poll_unregister(lease, window, cx);
        Ok(())
    }

    pub(super) fn unregister_released_subtree_by_lease(
        &self,
        lease: OverlayLayerLease,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let plan = self
            .state
            .update(cx, |state, _| state.begin_unregister_subtree(&lease))?;
        self.cancel_focus_claims(&plan.cancel_focus_claims, window, cx)?;
        for (lease, transition) in plan.removals {
            self.apply_focus_transition(transition, window, cx)?;
            self.poll_unregister(lease, window, cx);
        }
        Ok(())
    }

    /// Returns an immutable diagnostic projection.
    pub fn snapshot(
        &self,
        window: &Window,
        cx: &App,
    ) -> Result<WindowOverlaySnapshot, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        Ok(self.state.read(cx).snapshot(self.window_id))
    }

    pub(super) fn request_open_change_by_id(
        &self,
        layer_id: OverlayLayerId,
        lease_token: Option<u64>,
        open: bool,
        reason: DismissReason,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let plan = self.state.update(cx, |state, _| {
            state.request_open_change_plan(&layer_id, lease_token, open, reason)
        })?;
        self.cancel_focus_claims(&plan.cancel_focus_claims, window, cx)?;
        for dispatch in &plan.dispatches {
            self.apply_focus_transition(dispatch.focus_transition.clone(), window, cx)?;
        }
        self.run_open_change_dispatches(plan.dispatches, open, window, cx);
        Ok(plan.generation)
    }

    fn run_open_change_dispatches(
        &self,
        dispatches: Vec<OpenChangeDispatch>,
        open: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let changed = dispatches.iter().any(|dispatch| dispatch.changed);
        let mut callbacks = Vec::new();
        for dispatch in dispatches {
            if !self.state.read(cx).dispatch_is_current(
                &dispatch.layer_id,
                dispatch.lease_token,
                dispatch.generation,
                dispatch.registration_revision,
            ) {
                continue;
            }
            if let Some(commit) = dispatch.uncontrolled_commit {
                commit(open, window, cx);
            }
            if let Some(callback) = dispatch.on_open_change {
                callbacks.push((
                    dispatch.layer_id,
                    dispatch.lease_token,
                    dispatch.generation,
                    dispatch.registration_revision,
                    callback,
                ));
            }
        }
        for (layer_id, lease_token, generation, registration_revision, callback) in callbacks {
            if self.state.read(cx).dispatch_is_current(
                &layer_id,
                lease_token,
                generation,
                registration_revision,
            ) {
                callback(open, window, cx);
            }
        }
        if changed {
            window.refresh();
        }
    }

    pub(super) fn ensure_window(&self, window: &Window) -> Result<(), WindowOverlayRuntimeError> {
        (window.window_handle().window_id() == self.window_id)
            .then_some(())
            .ok_or(WindowOverlayRuntimeError::WrongWindow)
    }

    pub(super) fn ensure_binding(
        &self,
        binding: &OverlayLayerBinding,
        window: &Window,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        if binding.lease.window_id != self.window_id {
            return Err(WindowOverlayRuntimeError::WrongWindow);
        }
        Ok(())
    }
}
