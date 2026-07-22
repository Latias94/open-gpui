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

    /// Registers a new stable layer and returns its ownership binding.
    ///
    /// This is an explicit window-root boundary: its presentation does not inherit an element
    /// traversal. Retain the binding and either call [`Self::bind_layer_to_entity_release`] or
    /// explicitly unregister it.
    pub fn register_layer(
        &self,
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        let trigger_focus = cx.focus_handle();
        self.register_layer_with_focus_handles(
            registration,
            trigger_focus,
            SubtreePresentation::Visible,
            window,
            cx,
        )
    }

    pub(crate) fn register_layer_with_trigger_focus(
        &self,
        registration: OverlayLayerRegistration,
        trigger_focus: FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        self.register_layer_with_focus_handles(
            registration,
            trigger_focus,
            window.subtree_presentation(),
            window,
            cx,
        )
    }

    fn register_layer_with_focus_handles(
        &self,
        mut registration: OverlayLayerRegistration,
        trigger_focus: FocusHandle,
        presentation: SubtreePresentation,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        registration.presentation = presentation;
        validate_registration(&registration)?;
        let portal_anchor_registration = registration.portal_anchor;
        let portal_anchor = match portal_anchor_registration {
            PortalAnchorRegistration::None => None,
            PortalAnchorRegistration::RuntimeOwned => Some(window.new_portal_anchor()),
            PortalAnchorRegistration::External(handle) => {
                if handle.window_id() != self.window_id {
                    return Err(WindowOverlayRuntimeError::WrongWindow);
                }
                Some(handle)
            }
        };
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
                state.record_surface_focus_entered(&focus_layer_id, focus_lease);
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
            opening_theme: Rc::new(RefCell::new(None)),
            portal_anchor: portal_anchor.map(|handle| PortalOverlayAnchorBinding {
                handle,
                publication: open_gpui::PrepaintPublicationId::new(),
            }),
        })
    }

    /// Atomically registers a layer and binds its cleanup to an owner entity.
    pub(crate) fn register_layer_for_entity<T: 'static>(
        &self,
        registration: OverlayLayerRegistration,
        owner: &Entity<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        let trigger_focus = cx.focus_handle();
        let binding = self.register_layer_with_focus_handles(
            registration,
            trigger_focus,
            window.subtree_presentation(),
            window,
            cx,
        )?;
        if let Err(error) = self.bind_layer_to_entity_release(&binding, owner, window, cx) {
            let _ = self.unregister_layer(&binding, window, cx);
            return Err(error);
        }
        Ok(binding)
    }

    pub(crate) fn register_layer_for_entity_with_trigger_focus<T: 'static>(
        &self,
        registration: OverlayLayerRegistration,
        trigger_focus: FocusHandle,
        owner: &Entity<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        let binding =
            self.register_layer_with_trigger_focus(registration, trigger_focus, window, cx)?;
        if let Err(error) = self.bind_layer_to_entity_release(&binding, owner, window, cx) {
            let _ = self.unregister_layer(&binding, window, cx);
            return Err(error);
        }
        Ok(binding)
    }

    /// Rebinds an existing layer with its latest policy, ownership, and callbacks.
    ///
    /// This is an explicit window-root boundary. Stable parent identity and focus mode cannot
    /// change for the retained lease.
    pub fn rebind_layer(
        &self,
        binding: &OverlayLayerBinding,
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.rebind_layer_with_presentation(
            binding,
            registration,
            SubtreePresentation::Visible,
            window,
            cx,
        )
    }

    pub(crate) fn rebind_component_layer(
        &self,
        binding: &OverlayLayerBinding,
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.rebind_layer_with_presentation(
            binding,
            registration,
            window.subtree_presentation(),
            window,
            cx,
        )
    }

    fn rebind_layer_with_presentation(
        &self,
        binding: &OverlayLayerBinding,
        mut registration: OverlayLayerRegistration,
        presentation: SubtreePresentation,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        registration.presentation = presentation;
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
        for transition in plan.focus_transitions {
            self.apply_focus_transition(transition, window, cx)?;
        }
        self.run_open_change_dispatches(plan.descendant_dispatches, window, cx, |_, _| {});
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
        let window_handle = window.window_handle();
        let lease = binding.lease.clone();
        let subscription = cx.observe_release(owner, move |_, cx| {
            let weak_state = weak_state.clone();
            let lease = lease.clone();
            cx.defer(move |cx| {
                let Some(state) = weak_state.upgrade() else {
                    return;
                };
                let ambient_parent_layers = state.read(cx).ambient_parent_layers.clone();
                let runtime = WindowOverlayRuntime {
                    state,
                    window_id,
                    ambient_parent_layers,
                };
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = runtime.unregister_released_subtree_by_lease(lease, window, cx);
                });
            });
        });
        self.state.update(cx, |state, _| {
            let entry = state.entry_for_lease_mut(&binding.lease)?;
            entry.owner_entity = Some(owner.downgrade().into());
            entry.release_subscription = Some(subscription);
            Ok(())
        })
    }

    pub(super) fn replace_stale_component_subtree(
        &self,
        root: &OverlayLayerId,
        frame_revision: u64,
        owner_id: EntityId,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let Some(plan) = self.state.update(cx, |state, _| {
            state.begin_stale_component_subtree_replacement(
                root,
                self.window_id,
                frame_revision,
                owner_id,
            )
        }) else {
            return Ok(false);
        };
        self.cancel_focus_claims(&plan.cancel_focus_claims, window, cx)?;
        let mut leases = Vec::with_capacity(plan.removals.len());
        for (lease, transition) in plan.removals {
            self.apply_focus_transition(transition, window, cx)?;
            leases.push(lease);
        }
        for lease in leases {
            self.poll_unregister(lease, window, cx);
        }
        Ok(true)
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
        self.request_open_change_with_effect(binding, open, reason, window, cx, |_, _| {})
    }

    pub(crate) fn request_open_change_with_effect(
        &self,
        binding: &OverlayLayerBinding,
        open: bool,
        reason: DismissReason,
        window: &mut Window,
        cx: &mut App,
        effect: impl FnOnce(&mut Window, &mut App),
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.request_open_change_by_id_with_effect(
            binding.lease.layer_id.clone(),
            Some(binding.lease.token),
            open,
            reason,
            window,
            cx,
            effect,
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
        self.reject_controlled_intent_by_lease(&binding.lease, revision, window, cx)
    }

    pub(super) fn reject_controlled_intent_by_lease(
        &self,
        lease: &OverlayLayerLease,
        revision: OverlayOpenIntentRevision,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.reject_controlled_intent(lease, revision)
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
        let mut leases = Vec::with_capacity(plan.removals.len());
        for (lease, transition) in plan.removals {
            self.apply_focus_transition(transition, window, cx)?;
            leases.push(lease);
        }
        let runtime = self.clone();
        window.defer(cx, move |window, cx| {
            for lease in leases {
                runtime.poll_unregister(lease, window, cx);
            }
        });
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
        self.request_open_change_by_id_with_effect(
            layer_id,
            lease_token,
            open,
            reason,
            window,
            cx,
            |_, _| {},
        )
    }

    fn request_open_change_by_id_with_effect(
        &self,
        layer_id: OverlayLayerId,
        lease_token: Option<u64>,
        open: bool,
        reason: DismissReason,
        window: &mut Window,
        cx: &mut App,
        effect: impl FnOnce(&mut Window, &mut App),
    ) -> Result<OverlayLayerGeneration, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let plan = self.state.update(cx, |state, _| {
            state.request_open_change_plan(&layer_id, lease_token, open, reason)
        })?;
        self.cancel_focus_claims(&plan.cancel_focus_claims, window, cx)?;
        for dispatch in &plan.dispatches {
            self.apply_focus_transition(dispatch.focus_transition.clone(), window, cx)?;
        }
        self.run_open_change_dispatches(plan.dispatches, window, cx, effect);
        Ok(plan.generation)
    }

    pub(super) fn run_open_change_dispatches(
        &self,
        dispatches: Vec<OpenChangeDispatch>,
        window: &mut Window,
        cx: &mut App,
        effect: impl FnOnce(&mut Window, &mut App),
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
            let Some(intent) = dispatch.intent else {
                continue;
            };
            if let Some(commit) = dispatch.uncontrolled_commit {
                commit(intent.desired_open, window, cx);
            }
            if dispatch.notify_open_change {
                callbacks.push((
                    dispatch.layer_id,
                    dispatch.lease_token,
                    dispatch.open_change_revision,
                    dispatch.ownership,
                    intent,
                ));
            }
        }
        effect(window, cx);
        for (layer_id, lease_token, open_change_revision, ownership, intent) in callbacks {
            let callback = self.state.read(cx).current_open_change_callback(
                &layer_id,
                lease_token,
                open_change_revision,
                ownership,
                intent,
            );
            if let Some(callback) = callback {
                callback(
                    OverlayOpenIntent::new(
                        intent.desired_open,
                        intent.reason,
                        intent.revision,
                        self.clone(),
                        OverlayLayerLease {
                            layer_id,
                            token: lease_token,
                            window_id: self.window_id,
                        },
                    ),
                    window,
                    cx,
                );
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
