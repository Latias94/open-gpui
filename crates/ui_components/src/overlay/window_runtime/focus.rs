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
        let declared_target_id = registration.id().clone();
        let lease = self.state.update(cx, |state, _| {
            state.reserve_focus_target(&binding.lease, declared_target_id, self.window_id)
        })?;
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let scoped_registration = registration
            .assigned_id(lease.target_id.clone())
            .assigned_to_scope(Some(lease.scope_id.clone()));
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
        if registration.id() != target.declared_target_id() {
            return Err(WindowOverlayRuntimeError::FocusTargetIdChanged {
                expected: target.declared_target_id.clone(),
                actual: registration.id().clone(),
            });
        }
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        focus_runtime.rebind_target(
            registration
                .assigned_id(target.target_id.clone())
                .assigned_to_scope(Some(scope)),
            window,
            cx,
        )?;
        Ok(())
    }

    pub(super) fn sync_focus_target_set(
        &self,
        binding: &OverlayLayerBinding,
        current_targets: &[OverlayFocusTargetLease],
        registrations: Vec<FocusTargetRegistration>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<OverlayFocusTargetLease>, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        let scope = self
            .state
            .read(cx)
            .entries
            .get(&binding.lease.layer_id)
            .and_then(|entry| entry.scope_id.clone())
            .ok_or_else(|| {
                WindowOverlayRuntimeError::MissingFocusScope(binding.lease.layer_id.clone())
            })?;
        let mut current_by_declared_id = HashMap::with_capacity(current_targets.len());
        for target in current_targets {
            let target_scope = self.state.read(cx).validate_focus_target_lease(
                &binding.lease,
                target,
                self.window_id,
            )?;
            if target_scope != scope {
                return Err(WindowOverlayRuntimeError::ForeignFocusTargetLease(
                    target.declared_target_id.clone(),
                ));
            }
            if current_by_declared_id
                .insert(target.declared_target_id.clone(), target.clone())
                .is_some()
            {
                return Err(WindowOverlayRuntimeError::Focus(
                    FocusScopeRuntimeError::DuplicateTarget(target.declared_target_id.clone()),
                ));
            }
        }

        let mut desired_targets = Vec::with_capacity(registrations.len());
        let mut scoped_registrations = Vec::with_capacity(registrations.len());
        let mut newly_reserved = Vec::new();
        for registration in registrations {
            let declared_target_id = registration.id().clone();
            let target = if let Some(target) = current_by_declared_id.remove(&declared_target_id) {
                target
            } else {
                match self.state.update(cx, |state, _| {
                    state.reserve_focus_target(&binding.lease, declared_target_id, self.window_id)
                }) {
                    Ok(target) => {
                        newly_reserved.push(target.clone());
                        target
                    }
                    Err(error) => {
                        self.state.update(cx, |state, _| {
                            state.release_focus_target_reservations(&newly_reserved);
                        });
                        return Err(error);
                    }
                }
            };
            scoped_registrations.push(
                registration
                    .assigned_id(target.target_id.clone())
                    .assigned_to_scope(Some(scope.clone())),
            );
            desired_targets.push(target);
        }

        let stale_targets = current_by_declared_id.into_values().collect::<Vec<_>>();
        let previous_target_ids = current_targets
            .iter()
            .map(|target| target.target_id.clone())
            .collect::<Vec<_>>();
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        if let Err(error) = focus_runtime.sync_targets(
            &scope,
            &previous_target_ids,
            scoped_registrations,
            window,
            cx,
        ) {
            self.state.update(cx, |state, _| {
                state.release_focus_target_reservations(&newly_reserved);
            });
            return Err(error.into());
        }

        self.state.update(cx, |state, _| {
            state.release_focus_target_reservations(&stale_targets);
        });
        Ok(desired_targets)
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
        focus_runtime.unregister_target(&target.target_id, window, cx)?;
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
        let (trigger_registration, scope_registration, surface_registration) =
            focus_bundle(config, scope_id);
        focus_runtime.register_scope_bundle(
            trigger_registration,
            scope_registration,
            surface_registration,
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
        let (trigger_registration, scope_registration, surface_registration) =
            focus_bundle(config, scope_id);
        focus_runtime.rebind_scope_bundle(
            trigger_registration,
            scope_registration,
            surface_registration,
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
            FocusTransition::Resume(scope) => {
                focus_runtime.resume_scope(scope, window, cx)?;
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

    pub(super) fn retry_focus_claim_after_surface_prepaint(
        &self,
        binding: &OverlayLayerBinding,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        let scope = {
            let state = self.state.read(cx);
            state.validate_lease(&binding.lease)?;
            state.entries[&binding.lease.layer_id].scope_id.clone()
        };
        let Some(scope) = scope else {
            return Ok(());
        };
        let focus_runtime = self.state.read(cx).focus_runtime.clone();
        let retry = focus_runtime.retry_pending_claim_for_scope(&scope, window, cx)?;
        if retry.was_scheduled() {
            let weak_state = self.state.downgrade();
            let layer_id = binding.lease.layer_id.clone();
            let lease_token = binding.lease.token;
            let surface_focus = binding.surface_focus.clone();
            window.defer(cx, move |window, cx| {
                let Some(current) = window.focused(cx) else {
                    return;
                };
                if !surface_focus.contains(&current, window) {
                    return;
                }
                let _ = weak_state.update(cx, |state, _| {
                    state.record_surface_focus_entered(&layer_id, lease_token);
                });
            });
        } else if !retry.is_pending()
            && window
                .focused(cx)
                .is_some_and(|current| binding.surface_focus.contains(&current, window))
        {
            self.state.update(cx, |state, _| {
                state.record_surface_focus_entered(&binding.lease.layer_id, binding.lease.token);
            });
        }
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
        let pending_parent = self.state.read(cx).pending_unregister_parent_lease(&lease);
        let removed = self
            .state
            .update(cx, |state, _| state.take_unregistered(&lease));
        debug_assert!(
            removed.is_some(),
            "pending overlay cleanup must remain owned"
        );
        if let Some(parent) = pending_parent {
            self.poll_unregister(parent, window, cx);
        }
    }
}

fn focus_bundle(
    config: &LayerFocusConfig,
    scope_id: &FocusScopeId,
) -> (
    FocusTargetRegistration,
    FocusScopeRegistration,
    FocusTargetRegistration,
) {
    let initial_focus =
        canonical_initial_focus_intent(&config.layer_id, config.policy.initial_focus_intent());
    let mut scope_policy = FocusScopePolicy::new(scope_id.clone(), config.mode.into_scope_mode())
        .with_initial_focus(initial_focus)
        .with_focus_restore(config.policy.focus_restore_intent().clone());
    if let Some(parent_scope) = config.parent_scope.as_ref() {
        scope_policy = scope_policy.with_parent(parent_scope.clone());
    }
    (
        target_registration(
            config.trigger_id.clone(),
            &config.trigger_focus,
            config.parent_scope.as_ref(),
        ),
        FocusScopeRegistration::new(scope_policy, &config.surface_focus)
            .with_trigger(config.trigger_id.clone())
            .with_surface(config.surface_id.clone()),
        FocusTargetRegistration::new(config.surface_id.clone(), &config.surface_focus)
            .within_scope(scope_id.clone()),
    )
}

fn canonical_initial_focus_intent(
    layer: &OverlayLayerId,
    intent: &InitialFocusIntent,
) -> InitialFocusIntent {
    match intent {
        InitialFocusIntent::None => InitialFocusIntent::None,
        InitialFocusIntent::FirstFocusable => InitialFocusIntent::FirstFocusable,
        InitialFocusIntent::Target(target) => {
            InitialFocusIntent::Target(canonical_focus_target_id(layer, target))
        }
        InitialFocusIntent::TargetOrFirstFocusable(target) => {
            InitialFocusIntent::TargetOrFirstFocusable(canonical_focus_target_id(layer, target))
        }
    }
}

fn canonical_focus_target_id(layer: &OverlayLayerId, declared: &FocusTargetId) -> FocusTargetId {
    let layer = layer.as_str();
    let declared = declared.as_str();
    FocusTargetId::new(format!(
        "overlay-focus-target:{}:{}:{}:{}",
        layer.len(),
        layer,
        declared.len(),
        declared,
    ))
}

impl WindowOverlayRuntimeState {
    pub(super) fn record_surface_focus_entered(
        &mut self,
        layer_id: &OverlayLayerId,
        lease_token: u64,
    ) {
        if !self
            .entries
            .get(layer_id)
            .is_some_and(|entry| entry.lease_token == lease_token)
        {
            return;
        }

        let mut current = Some(layer_id.clone());
        while let Some(current_id) = current {
            let Some(entry) = self.entries.get_mut(&current_id) else {
                break;
            };
            entry.focus_entered = true;
            current = entry.parent.clone();
        }
    }

    pub(super) fn reserve_focus_target(
        &mut self,
        layer: &OverlayLayerLease,
        declared_target_id: FocusTargetId,
        window_id: WindowId,
    ) -> Result<OverlayFocusTargetLease, WindowOverlayRuntimeError> {
        self.validate_lease(layer)?;
        let target_id = canonical_focus_target_id(&layer.layer_id, &declared_target_id);
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
            declared_target_id,
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

    pub(super) fn release_focus_target_reservations(
        &mut self,
        targets: &[OverlayFocusTargetLease],
    ) {
        for target in targets {
            self.rollback_focus_target(target);
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
                target.declared_target_id.clone(),
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
                target.declared_target_id.clone(),
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
            layer_id: entry.id.clone(),
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
        let presentation = self.entries[id].presentation;
        self.lifecycle_presentation_transition(
            id,
            old_phase,
            next_phase,
            presentation,
            presentation,
        )
    }

    pub(super) fn lifecycle_presentation_transition(
        &self,
        id: &OverlayLayerId,
        old_phase: OverlayLayerPhase,
        next_phase: OverlayLayerPhase,
        old_presentation: SubtreePresentation,
        next_presentation: SubtreePresentation,
    ) -> FocusTransition {
        let entry = &self.entries[id];
        if entry.scope_id.is_none() {
            return FocusTransition::None;
        }
        let old_eligible = matches!(
            old_phase,
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested
        ) && old_presentation.is_interactive();
        let next_eligible = matches!(
            next_phase,
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested
        ) && next_presentation.is_interactive();
        if !old_eligible
            && next_eligible
            && old_phase != next_phase
            && next_phase == OverlayLayerPhase::Open
        {
            return FocusTransition::Activate(entry.scope_id.clone().expect("scope checked"));
        }
        if !old_eligible && next_eligible {
            return FocusTransition::Resume(entry.scope_id.clone().expect("scope checked"));
        }
        if old_eligible && !next_eligible {
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
            if parent_entry.policy.kind() != OverlayLayerKind::Menu {
                return Some(root);
            }
            root = parent.clone();
        }
    }
}
