//! Focus-scope registration, arbitration, and restoration.

use super::*;

impl WindowOverlayRuntime {
    /// Registers an additional logical focus target inside the layer's canonical scope.
    pub fn register_focus_target(
        &self,
        binding: &OverlayLayerBinding,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<OverlayFocusTargetLease, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        let target_id = registration.id().clone();
        let lease = self.state.update(cx, |state, _| {
            state.reserve_focus_target(&binding.lease, target_id, self.window_id)
        })?;
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let scoped_registration = registration.assigned_to_scope(Some(lease.scope_id.clone()));
        if let Err(error) = focus_runtime.register_target(scoped_registration, window, cx) {
            self.state.update(cx, |state, _| {
                state.rollback_focus_target(&lease);
            });
            return Err(error.into());
        }
        Ok(lease)
    }

    /// Rebinds an additional logical target to its latest live handle and availability.
    pub fn rebind_focus_target(
        &self,
        binding: &OverlayLayerBinding,
        target: &OverlayFocusTargetLease,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        let scope = self.state.read(cx).validate_focus_target_lease(
            &binding.lease,
            target,
            self.window_id,
        )?;
        if registration.id() != target.target_id() {
            return Err(WindowOverlayRuntimeError::FocusTargetIdChanged {
                expected: target.target_id.clone(),
                actual: registration.id().clone(),
            });
        }
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.rebind_target(registration.assigned_to_scope(Some(scope)), window, cx)?;
        Ok(())
    }

    /// Unregisters one additional logical focus target owned by this layer lease.
    pub fn unregister_focus_target(
        &self,
        binding: &OverlayLayerBinding,
        target: &OverlayFocusTargetLease,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state
            .read(cx)
            .validate_focus_target_lease(&binding.lease, target, self.window_id)?;
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.unregister_target(target.target_id(), window, cx)?;
        self.state.update(cx, |state, _| {
            state.remove_focus_target(&binding.lease, target)
        })
    }

    /// Registers the application fallback used when overlay trigger and ancestor targets vanish.
    pub fn register_window_fallback(
        &self,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<WindowFocusFallbackLease, WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        let target_id = registration.id().clone();
        let lease = self.state.update(cx, |state, _| {
            state.reserve_window_fallback(target_id, self.window_id)
        })?;
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        if let Err(error) = focus_runtime.register_window_fallback_target(
            registration.assigned_to_scope(None),
            window,
            cx,
        ) {
            self.state.update(cx, |state, _| {
                state.rollback_window_fallback(&lease);
            });
            return Err(error.into());
        }
        Ok(lease)
    }

    /// Rebinds the current window fallback to its latest live handle and availability.
    pub fn rebind_window_fallback(
        &self,
        fallback: &WindowFocusFallbackLease,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .read(cx)
            .validate_window_fallback_lease(fallback, self.window_id)?;
        if registration.id() != fallback.target_id() {
            return Err(WindowOverlayRuntimeError::FocusTargetIdChanged {
                expected: fallback.target_id.clone(),
                actual: registration.id().clone(),
            });
        }
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.rebind_window_fallback_target(
            registration.assigned_to_scope(None),
            window,
            cx,
        )?;
        Ok(())
    }

    /// Clears and unregisters the current application fallback target.
    pub fn unregister_window_fallback(
        &self,
        fallback: &WindowFocusFallbackLease,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .read(cx)
            .validate_window_fallback_lease(fallback, self.window_id)?;
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.unregister_window_fallback_target(fallback.target_id(), window, cx)?;
        self.state
            .update(cx, |state, _| state.remove_window_fallback(fallback))
    }

    pub(super) fn install_focus(
        &self,
        config: &LayerFocusConfig,
        activate: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let Some(scope_id) = config.scope_id.as_ref() else {
            return Ok(());
        };
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let trigger_registration = target_registration(
            config.trigger_id.clone(),
            &config.trigger_focus,
            config.parent_scope.as_ref(),
        );
        let mut scope_policy =
            FocusScopePolicy::new(scope_id.clone(), config.mode.into_scope_mode())
                .with_initial_focus(config.policy.initial_focus_intent().clone())
                .with_focus_restore(config.policy.focus_restore_intent().clone());
        if let Some(parent_scope) = config.parent_scope.as_ref() {
            scope_policy = scope_policy.with_parent(parent_scope.clone());
        }
        focus_runtime.register_scope_bundle(
            trigger_registration,
            FocusScopeRegistration::new(scope_policy, &config.surface_focus)
                .with_surface(config.surface_id.clone()),
            FocusTargetRegistration::new(config.surface_id.clone(), &config.surface_focus)
                .within_scope(scope_id.clone()),
            window,
            cx,
        )?;
        if activate {
            if let Err(error) = focus_runtime.activate_scope(scope_id.clone(), window, cx) {
                focus_runtime
                    .unregister_scope_bundle(&config.trigger_id, scope_id, window, cx)
                    .expect("new overlay focus bundle must remain available for rollback");
                return Err(error.into());
            }
        }
        Ok(())
    }

    pub(super) fn rebind_focus(
        &self,
        config: &LayerFocusConfig,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let Some(scope_id) = config.scope_id.as_ref() else {
            return Ok(());
        };
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let mut scope_policy =
            FocusScopePolicy::new(scope_id.clone(), config.mode.into_scope_mode())
                .with_initial_focus(config.policy.initial_focus_intent().clone())
                .with_focus_restore(config.policy.focus_restore_intent().clone());
        if let Some(parent_scope) = config.parent_scope.as_ref() {
            scope_policy = scope_policy.with_parent(parent_scope.clone());
        }
        focus_runtime.rebind_scope_bundle(
            target_registration(
                config.trigger_id.clone(),
                &config.trigger_focus,
                config.parent_scope.as_ref(),
            ),
            FocusScopeRegistration::new(scope_policy, &config.surface_focus)
                .with_surface(config.surface_id.clone()),
            FocusTargetRegistration::new(config.surface_id.clone(), &config.surface_focus)
                .within_scope(scope_id.clone()),
            window,
            cx,
        )?;
        Ok(())
    }

    pub(super) fn apply_focus_transition(
        &self,
        transition: FocusTransition,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        match transition {
            FocusTransition::None => Ok(()),
            FocusTransition::Activate(scope) => {
                focus_runtime.activate_scope(scope, window, cx)?;
                Ok(())
            }
            FocusTransition::Deactivate { scope, restore } => {
                focus_runtime.deactivate_scope_with_restore(scope, restore, window, cx)?;
                Ok(())
            }
        }
    }

    pub(super) fn cancel_focus_claims(
        &self,
        scopes: &[FocusScopeId],
        window: &Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        if scopes.is_empty() {
            return Ok(());
        }
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.cancel_claims_for_scopes(scopes, window, cx)?;
        Ok(())
    }

    pub(super) fn poll_unregister(
        &self,
        lease: OverlayLayerLease,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let cleanup = self.state.read(cx).pending_unregister_cleanup(&lease);
        let Some(cleanup) = cleanup else {
            return;
        };
        if self.state.read(cx).has_registered_children(&lease.layer_id) {
            let runtime = self.clone();
            window.on_next_frame(move |window, cx| runtime.poll_unregister(lease, window, cx));
            window.refresh();
            return;
        }
        let pending = cleanup.scope_id.as_ref().is_some_and(|scope| {
            focus_runtime
                .has_pending_claim_for_scope(scope, window, cx)
                .expect("overlay focus runtime must belong to the same window")
        });
        if pending {
            let runtime = self.clone();
            window.on_next_frame(move |window, cx| runtime.poll_unregister(lease, window, cx));
            window.refresh();
            return;
        }

        if let Some(scope_id) = cleanup.scope_id.as_ref() {
            focus_runtime
                .unregister_scope_bundle(&cleanup.trigger_id, scope_id, window, cx)
                .expect("overlay focus bundle must remain registered until layer cleanup");
        }
        let removed = self
            .state
            .update(cx, |state, _| state.take_unregistered(&lease));
        debug_assert!(
            removed.is_some(),
            "pending overlay cleanup must remain owned"
        );
    }
}

impl WindowOverlayRuntimeState {
    pub(super) fn reserve_focus_target(
        &mut self,
        layer: &OverlayLayerLease,
        target_id: FocusTargetId,
        window_id: WindowId,
    ) -> Result<OverlayFocusTargetLease, WindowOverlayRuntimeError> {
        self.validate_lease(layer)?;
        let entry = self
            .entries
            .get(&layer.layer_id)
            .expect("overlay lease was validated before reserving a focus target");
        if entry.pending_unregister {
            return Err(WindowOverlayRuntimeError::LayerUnregistering(
                layer.layer_id.clone(),
            ));
        }
        let Some(scope_id) = entry.scope_id.clone() else {
            return Err(WindowOverlayRuntimeError::MissingFocusScope(
                layer.layer_id.clone(),
            ));
        };
        if self.entries.values().any(|entry| {
            entry.trigger_id == target_id
                || entry.surface_id == target_id
                || entry.focus_targets.contains_key(&target_id)
        }) || self
            .window_fallback
            .as_ref()
            .is_some_and(|fallback| fallback.target_id == target_id)
        {
            return Err(WindowOverlayRuntimeError::Focus(
                FocusScopeRuntimeError::DuplicateTarget(target_id),
            ));
        }
        self.next_focus_target_token = self.next_focus_target_token.wrapping_add(1);
        let target_token = self.next_focus_target_token;
        self.entries
            .get_mut(&layer.layer_id)
            .expect("overlay lease was validated before reserving a focus target")
            .focus_targets
            .insert(target_id.clone(), target_token);
        Ok(OverlayFocusTargetLease {
            window_id,
            layer_id: layer.layer_id.clone(),
            layer_token: layer.token,
            scope_id,
            target_id,
            target_token,
        })
    }

    pub(super) fn rollback_focus_target(&mut self, target: &OverlayFocusTargetLease) {
        if let Some(entry) = self.entries.get_mut(&target.layer_id)
            && entry.lease_token == target.layer_token
            && entry.focus_targets.get(&target.target_id) == Some(&target.target_token)
        {
            entry.focus_targets.remove(&target.target_id);
        }
    }

    pub(super) fn validate_focus_target_lease(
        &self,
        layer: &OverlayLayerLease,
        target: &OverlayFocusTargetLease,
        window_id: WindowId,
    ) -> Result<FocusScopeId, WindowOverlayRuntimeError> {
        self.validate_mutable_lease(layer)?;
        if target.window_id != window_id
            || target.layer_id != layer.layer_id
            || target.layer_token != layer.token
        {
            return Err(WindowOverlayRuntimeError::ForeignFocusTargetLease(
                target.target_id.clone(),
            ));
        }
        let entry = self
            .entries
            .get(&layer.layer_id)
            .expect("overlay lease was validated before checking a focus target");
        if entry.scope_id.as_ref() != Some(&target.scope_id)
            || entry.focus_targets.get(&target.target_id) != Some(&target.target_token)
        {
            return Err(WindowOverlayRuntimeError::ForeignFocusTargetLease(
                target.target_id.clone(),
            ));
        }
        Ok(target.scope_id.clone())
    }

    pub(super) fn remove_focus_target(
        &mut self,
        layer: &OverlayLayerLease,
        target: &OverlayFocusTargetLease,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_focus_target_lease(layer, target, target.window_id)?;
        self.entries
            .get_mut(&layer.layer_id)
            .expect("overlay target lease was validated before removal")
            .focus_targets
            .remove(&target.target_id);
        Ok(())
    }

    pub(super) fn reserve_window_fallback(
        &mut self,
        target_id: FocusTargetId,
        window_id: WindowId,
    ) -> Result<WindowFocusFallbackLease, WindowOverlayRuntimeError> {
        if let Some(existing) = self.window_fallback.as_ref() {
            return Err(WindowOverlayRuntimeError::DuplicateWindowFallback(
                existing.target_id.clone(),
            ));
        }
        self.next_focus_target_token = self.next_focus_target_token.wrapping_add(1);
        let target_token = self.next_focus_target_token;
        self.window_fallback = Some(WindowFallbackEntry {
            target_id: target_id.clone(),
            target_token,
        });
        Ok(WindowFocusFallbackLease {
            window_id,
            target_id,
            target_token,
        })
    }

    pub(super) fn rollback_window_fallback(&mut self, fallback: &WindowFocusFallbackLease) {
        if self.window_fallback.as_ref().is_some_and(|entry| {
            entry.target_id == fallback.target_id && entry.target_token == fallback.target_token
        }) {
            self.window_fallback = None;
        }
    }

    pub(super) fn validate_window_fallback_lease(
        &self,
        fallback: &WindowFocusFallbackLease,
        window_id: WindowId,
    ) -> Result<(), WindowOverlayRuntimeError> {
        if fallback.window_id != window_id
            || !self.window_fallback.as_ref().is_some_and(|entry| {
                entry.target_id == fallback.target_id && entry.target_token == fallback.target_token
            })
        {
            return Err(WindowOverlayRuntimeError::ForeignWindowFallbackLease(
                fallback.target_id.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn remove_window_fallback(
        &mut self,
        fallback: &WindowFocusFallbackLease,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_window_fallback_lease(fallback, fallback.window_id)?;
        self.window_fallback = None;
        Ok(())
    }

    pub(super) fn focus_config(
        &self,
        id: &OverlayLayerId,
        trigger_focus: &FocusHandle,
        surface_focus: &FocusHandle,
    ) -> Result<LayerFocusConfig, WindowOverlayRuntimeError> {
        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| WindowOverlayRuntimeError::UnknownLayer(id.clone()))?;
        Ok(LayerFocusConfig {
            mode: entry.focus_mode,
            policy: entry.projected_policy(),
            scope_id: entry.scope_id.clone(),
            parent_scope: self.nearest_parent_scope(entry.parent.as_ref()),
            trigger_id: entry.trigger_id.clone(),
            surface_id: entry.surface_id.clone(),
            trigger_focus: trigger_focus.clone(),
            surface_focus: surface_focus.clone(),
        })
    }

    pub(super) fn nearest_parent_scope(
        &self,
        parent: Option<&OverlayLayerId>,
    ) -> Option<FocusScopeId> {
        let mut current = parent;
        while let Some(id) = current {
            let entry = self.entries.get(id)?;
            if let Some(scope) = entry.scope_id.as_ref() {
                return Some(scope.clone());
            }
            current = entry.parent.as_ref();
        }
        None
    }

    pub(super) fn lifecycle_transition(
        &self,
        id: &OverlayLayerId,
        old_phase: OverlayLayerPhase,
        next_phase: OverlayLayerPhase,
    ) -> FocusTransition {
        let entry = &self.entries[id];
        if entry.scope_id.is_none() {
            return FocusTransition::None;
        }
        if !matches!(
            old_phase,
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested
        ) && next_phase == OverlayLayerPhase::Open
        {
            return FocusTransition::Activate(entry.scope_id.clone().expect("scope checked"));
        }
        if matches!(
            old_phase,
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested
        ) && matches!(
            next_phase,
            OverlayLayerPhase::Closing | OverlayLayerPhase::Hidden
        ) {
            return FocusTransition::Deactivate {
                scope: entry.scope_id.clone().expect("scope checked"),
                restore: entry.should_restore_focus(),
            };
        }
        FocusTransition::None
    }

    pub(super) fn subtree_restore_owner(
        &self,
        root: &OverlayLayerId,
        layer_ids: &[OverlayLayerId],
    ) -> Option<OverlayLayerId> {
        self.entries.get(root)?;
        layer_ids
            .iter()
            .filter_map(|layer_id| {
                let entry = self.entries.get(layer_id)?;
                (entry.scope_id.is_some() && entry.focus_active && entry.should_restore_focus())
                    .then(|| (self.layer_depth(layer_id), layer_id.clone()))
            })
            .max_by_key(|(depth, layer_id)| {
                (
                    Reverse(*depth),
                    self.stack
                        .iter()
                        .position(|candidate| candidate == layer_id)
                        .unwrap_or(0),
                )
            })
            .map(|(_, layer_id)| layer_id)
    }

    pub(super) fn subtree_focus_claim_cancellations(
        &self,
        layer_ids: &[OverlayLayerId],
        restore_owner: Option<&OverlayLayerId>,
    ) -> Vec<FocusScopeId> {
        layer_ids
            .iter()
            .filter(|layer_id| restore_owner != Some(*layer_id))
            .filter_map(|layer_id| self.entries.get(layer_id)?.scope_id.clone())
            .collect()
    }

    pub(super) fn tab_dismiss_target(&self) -> Option<OverlayLayerId> {
        for id in self.stack.iter().rev() {
            let Some(entry) = self
                .entries
                .get(id)
                .filter(|entry| entry.keyboard_eligible())
            else {
                continue;
            };
            match &entry.tab_behavior {
                OverlayTabBehavior::Preserve if entry.focus_mode == OverlayFocusMode::None => {
                    continue;
                }
                OverlayTabBehavior::Preserve => return None,
                OverlayTabBehavior::DismissSelf => return Some(id.clone()),
                OverlayTabBehavior::DismissMenuRoot => return self.menu_root(id),
            }
        }
        None
    }

    fn menu_root(&self, id: &OverlayLayerId) -> Option<OverlayLayerId> {
        let mut root = id.clone();
        loop {
            let entry = self
                .entries
                .get(&root)
                .filter(|entry| entry.keyboard_eligible())?;
            let Some(parent) = entry.parent.as_ref() else {
                return Some(root);
            };
            let Some(parent_entry) = self.entries.get(parent) else {
                return None;
            };
            if parent_entry.policy.kind != OverlayLayerKind::Menu {
                return Some(root);
            }
            root = parent.clone();
        }
    }
}
