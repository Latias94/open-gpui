//! Layer lifecycle state machine, hierarchy validation, and stack projection.

use std::collections::HashSet;

use super::*;

impl WindowOverlayRuntimeState {
    pub(super) fn new(window: &Window, cx: &mut App) -> Self {
        Self {
            focus_runtime: FocusScopeRuntime::new(window, cx),
            ambient_parent_layers: Rc::new(RefCell::new(Vec::new())),
            entries: HashMap::new(),
            registration_order: Vec::new(),
            stack: Vec::new(),
            next_lease_token: 0,
            next_focus_target_token: 0,
            window_fallback: None,
            key_subscription: None,
            mouse_subscription: None,
            activation_subscription: None,
            mouse_routes: HashMap::new(),
            mouse_authority_revision: 0,
        }
    }

    pub(super) fn accessibility_tree_scope(
        &self,
        lease: &OverlayLayerLease,
        runtime_window_id: WindowId,
    ) -> AccessibilityTreeScope {
        let active_modal = self.stack.iter().rev().find(|layer_id| {
            self.entries.get(*layer_id).is_some_and(|entry| {
                entry.policy.kind() == OverlayLayerKind::Modal
                    && entry.keyboard_eligible()
                    && !entry.pending_unregister
            })
        });
        let surface = (lease.window_id == runtime_window_id)
            .then(|| self.entries.get(&lease.layer_id))
            .flatten()
            .filter(|entry| {
                entry.lease_token == lease.token
                    && entry.keyboard_eligible()
                    && !entry.pending_unregister
            });

        match (active_modal, surface) {
            (None, Some(_)) => AccessibilityTreeScope::Unrestricted,
            (Some(modal_id), Some(entry)) if &entry.id == modal_id => {
                AccessibilityTreeScope::ModalRoot
            }
            (Some(modal_id), Some(entry)) if self.is_descendant_or_same(modal_id, &entry.id) => {
                AccessibilityTreeScope::ModalDescendant
            }
            _ => AccessibilityTreeScope::Excluded,
        }
    }

    pub(super) fn register_layer(
        &mut self,
        registration: OverlayLayerRegistration,
        trigger_focus: &FocusHandle,
        surface_focus: &FocusHandle,
        window_id: WindowId,
    ) -> Result<(OverlayLayerLease, LayerFocusConfig, bool), WindowOverlayRuntimeError> {
        if self.entries.contains_key(&registration.id) {
            return Err(WindowOverlayRuntimeError::DuplicateLayer(registration.id));
        }
        self.validate_parent(&registration.id, registration.parent.as_ref())?;
        let lifecycle = LayerLifecycle::from_presence(registration.policy.presence());
        let phase = lifecycle.phase();
        let presentation = registration
            .parent
            .as_ref()
            .and_then(|parent| self.entries.get(parent))
            .map(|parent| registration.presentation.max(parent.presentation))
            .unwrap_or(registration.presentation);
        let mouse_authority_changed =
            MouseAuthorityProfile::from_policy(&registration.policy, phase, presentation)
                .affects_routing();
        self.validate_parent_lifecycle(registration.parent.as_ref(), phase)?;
        self.next_lease_token = self.next_lease_token.wrapping_add(1);
        let lease_token = self.next_lease_token;
        let generation = OverlayLayerGeneration(1);
        let focus_active = phase == OverlayLayerPhase::Open
            && registration.focus_mode != OverlayFocusMode::None
            && presentation.is_interactive();
        let scope_id = (registration.focus_mode != OverlayFocusMode::None)
            .then(|| scope_id_for(&registration.id));
        let trigger_id = trigger_id_for(&registration.id);
        let surface_id = surface_id_for(&registration.id);
        let id = registration.id.clone();
        let entry = LayerEntry {
            id: id.clone(),
            parent: registration.parent,
            policy: registration.policy,
            ownership: registration.ownership,
            focus_mode: registration.focus_mode,
            focus_restore_condition: registration.focus_restore_condition,
            tab_behavior: registration.tab_behavior,
            on_open_change: registration.on_open_change,
            uncontrolled_commit: registration.uncontrolled_commit,
            lease_token,
            lifecycle,
            local_presentation: registration.presentation,
            presentation,
            generation,
            registration_revision: 1,
            open_change_revision: 0,
            focus_active,
            focus_entered: false,
            scope_id,
            trigger_id,
            surface_id,
            focus_targets: HashMap::new(),
            inside_regions: HashMap::new(),
            focus_subscription: None,
            release_subscription: None,
            owner_entity: None,
            component_bind_revision: None,
            pending_unregister: false,
            forced_by_ancestor: false,
        };
        self.entries.insert(id.clone(), entry);
        self.registration_order.push(id.clone());
        if phase != OverlayLayerPhase::Hidden {
            self.stack.push(id.clone());
        }
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        let focus_config = self.focus_config(&id, trigger_focus, surface_focus)?;
        Ok((
            OverlayLayerLease {
                layer_id: id,
                token: lease_token,
                window_id,
            },
            focus_config,
            focus_active,
        ))
    }

    pub(super) fn rebind_layer_plan(
        &mut self,
        lease: &OverlayLayerLease,
        registration: OverlayLayerRegistration,
    ) -> Result<RebindPlan, WindowOverlayRuntimeError> {
        self.validate_rebind(lease, &registration)?;
        let requested_phase = LayerLifecycle::from_presence(registration.policy.presence()).phase();
        let current_phase = self.entries[&registration.id].lifecycle.phase();
        let closes_subtree = current_phase != OverlayLayerPhase::Hidden
            && matches!(
                requested_phase,
                OverlayLayerPhase::Closing | OverlayLayerPhase::Hidden
            )
            && self.has_present_descendants(&registration.id);
        if !closes_subtree {
            let transition = self.rebind_layer(lease, registration)?;
            return Ok(RebindPlan {
                generation: transition.generation,
                cancel_focus_claims: Vec::new(),
                focus_transitions: transition
                    .focus_transitions
                    .into_iter()
                    .map(|planned| planned.transition)
                    .collect(),
                descendant_dispatches: Vec::new(),
            });
        }

        let root_id = registration.id.clone();
        let mut descendant_ids = self
            .registration_order
            .iter()
            .filter(|candidate| {
                **candidate != root_id
                    && self.is_descendant_or_same(&root_id, candidate)
                    && self.entries[*candidate].lifecycle.presence().present()
            })
            .cloned()
            .collect::<Vec<_>>();
        descendant_ids.sort_by(|left, right| {
            self.teardown_order_key(right)
                .cmp(&self.teardown_order_key(left))
        });
        let mut subtree_ids = descendant_ids.clone();
        subtree_ids.push(root_id.clone());
        let restore_owner = self.subtree_restore_owner(&root_id, &subtree_ids);
        let cancel_focus_claims =
            self.subtree_focus_claim_cancellations(&subtree_ids, restore_owner.as_ref());
        let mut descendant_dispatches = Vec::with_capacity(descendant_ids.len());
        for descendant_id in descendant_ids {
            descendant_dispatches.push(self.force_close_for_ancestor(
                &descendant_id,
                DismissReason::Programmatic,
                restore_owner.as_ref() == Some(&descendant_id),
                requested_phase,
                true,
            )?);
        }

        let mut transition = self.rebind_layer(lease, registration)?;
        if restore_owner.as_ref() != Some(&root_id) {
            for planned in &mut transition.focus_transitions {
                if planned.layer_id == root_id
                    && let FocusTransition::Deactivate { restore, .. } = &mut planned.transition
                {
                    *restore = false;
                }
            }
        }
        Ok(RebindPlan {
            generation: transition.generation,
            cancel_focus_claims,
            focus_transitions: transition
                .focus_transitions
                .into_iter()
                .map(|planned| planned.transition)
                .collect(),
            descendant_dispatches,
        })
    }

    pub(super) fn rebind_layer(
        &mut self,
        lease: &OverlayLayerLease,
        registration: OverlayLayerRegistration,
    ) -> Result<RebindTransition, WindowOverlayRuntimeError> {
        self.validate_rebind(lease, &registration)?;

        let root_id = registration.id.clone();
        let old_phase = self.entries[&root_id].lifecycle.phase();
        let ownership_changed = self.entries[&registration.id].ownership != registration.ownership;
        let mut next_lifecycle = self.entries[&registration.id].lifecycle.clone();
        if ownership_changed {
            next_lifecycle.rebind_ownership(registration.policy.presence());
        } else {
            next_lifecycle.rebind_presence(registration.policy.presence());
        }

        let mut affected = self
            .registration_order
            .iter()
            .filter(|candidate| self.is_descendant_or_same(&root_id, candidate))
            .cloned()
            .collect::<Vec<_>>();
        affected.sort_by_key(|id| self.layer_depth(id));

        let mut next_presentations = HashMap::with_capacity(affected.len());
        for id in &affected {
            let entry = &self.entries[id];
            let local = if id == &root_id {
                registration.presentation
            } else {
                entry.local_presentation
            };
            let inherited = entry.parent.as_ref().and_then(|parent| {
                next_presentations
                    .get(parent)
                    .copied()
                    .or_else(|| self.entries.get(parent).map(|entry| entry.presentation))
            });
            next_presentations.insert(
                id.clone(),
                inherited.map(|parent| local.max(parent)).unwrap_or(local),
            );
        }

        let root_presentation_suppressed = self.entries[&root_id].presentation.is_interactive()
            && !next_presentations[&root_id].is_interactive();
        if root_presentation_suppressed {
            next_lifecycle.discard_pending_for_suppression();
        }
        let next_phase = next_lifecycle.phase();

        let mut focus_transitions = Vec::new();
        let mut mouse_authority_changed = false;
        for id in &affected {
            let entry = &self.entries[id];
            let old_entry_phase = entry.lifecycle.phase();
            let presentation_suppressed =
                entry.presentation.is_interactive() && !next_presentations[id].is_interactive();
            let next_entry_phase = if id == &root_id {
                next_phase
            } else if presentation_suppressed
                && old_entry_phase == OverlayLayerPhase::CloseRequested
            {
                OverlayLayerPhase::Open
            } else {
                old_entry_phase
            };
            let next_presentation = next_presentations[id];
            let transition = self.lifecycle_presentation_transition(
                id,
                old_entry_phase,
                next_entry_phase,
                entry.presentation,
                next_presentation,
            );
            if !matches!(transition, FocusTransition::None) {
                focus_transitions.push(PlannedFocusTransition {
                    layer_id: id.clone(),
                    transition,
                    depth: self.layer_depth(id),
                });
            }

            let next_policy = if id == &root_id {
                &registration.policy
            } else {
                &entry.policy
            };
            mouse_authority_changed |= MouseAuthorityProfile::from_policy(
                &entry.policy,
                old_entry_phase,
                entry.presentation,
            ) != MouseAuthorityProfile::from_policy(
                next_policy,
                next_entry_phase,
                next_presentation,
            );
        }

        let entry = self
            .entries
            .get_mut(&registration.id)
            .expect("overlay lease was validated before rebind");
        let reopened = matches!(
            old_phase,
            OverlayLayerPhase::Closing | OverlayLayerPhase::Hidden
        ) && next_phase == OverlayLayerPhase::Open;
        if old_phase != next_phase || reopened {
            entry.generation = OverlayLayerGeneration(entry.generation.0.wrapping_add(1));
        }
        entry.parent = registration.parent;
        entry.policy = registration.policy;
        entry.ownership = registration.ownership;
        entry.focus_mode = registration.focus_mode;
        entry.focus_restore_condition = registration.focus_restore_condition;
        entry.tab_behavior = registration.tab_behavior;
        entry.on_open_change = registration.on_open_change;
        entry.uncontrolled_commit = registration.uncontrolled_commit;
        entry.registration_revision = entry.registration_revision.wrapping_add(1);
        entry.lifecycle = next_lifecycle;
        entry.local_presentation = registration.presentation;
        if next_phase == OverlayLayerPhase::Open {
            entry.forced_by_ancestor = false;
        }
        if reopened {
            entry.focus_entered = false;
            entry.pending_unregister = false;
        }
        for id in &affected {
            let entry = self
                .entries
                .get_mut(id)
                .expect("presentation descendants were collected from registered layers");
            let next_presentation = next_presentations[id];
            if entry.presentation.is_interactive() && !next_presentation.is_interactive() {
                entry.lifecycle.discard_pending_for_suppression();
            }
            entry.presentation = next_presentation;
        }
        for planned in &focus_transitions {
            let entry = self
                .entries
                .get_mut(&planned.layer_id)
                .expect("focus transition layer remains registered during rebind");
            match &planned.transition {
                FocusTransition::Activate(_) | FocusTransition::Resume(_) => {
                    entry.focus_active = true;
                }
                FocusTransition::Deactivate { .. } => entry.focus_active = false,
                FocusTransition::None => {}
            }
        }
        focus_transitions.sort_by(|left, right| {
            let left_deactivates = matches!(left.transition, FocusTransition::Deactivate { .. });
            let right_deactivates = matches!(right.transition, FocusTransition::Deactivate { .. });
            left_deactivates
                .cmp(&right_deactivates)
                .reverse()
                .then_with(|| {
                    if left_deactivates {
                        right.depth.cmp(&left.depth)
                    } else {
                        left.depth.cmp(&right.depth)
                    }
                })
        });
        self.sync_stack(&registration.id, old_phase, next_phase, reopened);
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        Ok(RebindTransition {
            focus_transitions,
            generation: self.entries[&registration.id].generation,
        })
    }

    pub(super) fn prepare_rebind(
        &self,
        lease: &OverlayLayerLease,
        registration: &OverlayLayerRegistration,
        trigger_focus: &FocusHandle,
        surface_focus: &FocusHandle,
    ) -> Result<LayerFocusConfig, WindowOverlayRuntimeError> {
        self.validate_rebind(lease, registration)?;
        let entry = self
            .entries
            .get(&registration.id)
            .expect("overlay lease was validated before preparing rebind");
        Ok(LayerFocusConfig {
            layer_id: entry.id.clone(),
            mode: registration.focus_mode,
            policy: registration.policy.clone(),
            scope_id: entry.scope_id.clone(),
            parent_scope: self.nearest_parent_scope(registration.parent.as_ref()),
            trigger_id: entry.trigger_id.clone(),
            surface_id: entry.surface_id.clone(),
            trigger_focus: trigger_focus.clone(),
            surface_focus: surface_focus.clone(),
        })
    }

    pub(super) fn validate_rebind(
        &self,
        lease: &OverlayLayerLease,
        registration: &OverlayLayerRegistration,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_lease(lease)?;
        let entry = self
            .entries
            .get(&registration.id)
            .ok_or_else(|| WindowOverlayRuntimeError::UnknownLayer(registration.id.clone()))?;
        if entry.pending_unregister {
            return Err(WindowOverlayRuntimeError::LayerUnregistering(
                registration.id.clone(),
            ));
        }
        if entry.focus_mode != registration.focus_mode {
            return Err(WindowOverlayRuntimeError::FocusModeChanged(
                registration.id.clone(),
            ));
        }
        self.validate_parent(&registration.id, registration.parent.as_ref())?;
        if entry.parent != registration.parent {
            return Err(WindowOverlayRuntimeError::ParentChanged(
                registration.id.clone(),
            ));
        }

        let next_phase = LayerLifecycle::from_presence(registration.policy.presence()).phase();
        self.validate_parent_lifecycle(registration.parent.as_ref(), next_phase)?;
        Ok(())
    }

    pub(super) fn request_open_change_plan(
        &mut self,
        id: &OverlayLayerId,
        lease_token: Option<u64>,
        open: bool,
        reason: DismissReason,
    ) -> Result<OpenChangePlan, WindowOverlayRuntimeError> {
        self.validate_request_target(id, lease_token)?;
        let cascade = !open
            && self.entries[id].ownership == OverlayOwnership::Uncontrolled
            && self.has_present_descendants(id);
        if !cascade {
            let dispatch = self.request_open_change(id, lease_token, open, reason)?;
            return Ok(OpenChangePlan {
                generation: dispatch.generation,
                cancel_focus_claims: Vec::new(),
                dispatches: vec![dispatch],
            });
        }

        let mut layer_ids = self
            .registration_order
            .iter()
            .filter(|candidate| {
                self.is_descendant_or_same(id, candidate)
                    && self.entries[*candidate].lifecycle.presence().present()
            })
            .cloned()
            .collect::<Vec<_>>();
        layer_ids.sort_by(|left, right| {
            self.teardown_order_key(right)
                .cmp(&self.teardown_order_key(left))
        });

        let restore_owner = self.subtree_restore_owner(id, &layer_ids);
        let cancel_focus_claims =
            self.subtree_focus_claim_cancellations(&layer_ids, restore_owner.as_ref());
        let mut generation = self.entries[id].generation;
        let mut dispatches = Vec::with_capacity(layer_ids.len());
        for layer_id in layer_ids {
            let is_root = layer_id == *id;
            let dispatch = self.force_close_for_ancestor(
                &layer_id,
                reason,
                restore_owner.as_ref() == Some(&layer_id),
                OverlayLayerPhase::Closing,
                !is_root,
            )?;
            if is_root {
                generation = dispatch.generation;
            }
            dispatches.push(dispatch);
        }
        Ok(OpenChangePlan {
            generation,
            cancel_focus_claims,
            dispatches,
        })
    }

    pub(super) fn validate_request_target(
        &self,
        id: &OverlayLayerId,
        lease_token: Option<u64>,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let Some(entry) = self.entries.get(id) else {
            return Err(WindowOverlayRuntimeError::UnknownLayer(id.clone()));
        };
        if let Some(token) = lease_token
            && entry.lease_token != token
        {
            return Err(WindowOverlayRuntimeError::ForeignLease(id.clone()));
        }
        if entry.pending_unregister {
            return Err(WindowOverlayRuntimeError::LayerUnregistering(id.clone()));
        }
        Ok(())
    }

    pub(super) fn request_open_change(
        &mut self,
        id: &OverlayLayerId,
        lease_token: Option<u64>,
        open: bool,
        reason: DismissReason,
    ) -> Result<OpenChangeDispatch, WindowOverlayRuntimeError> {
        self.validate_request_target(id, lease_token)?;
        let entry = &self.entries[id];
        if open == entry.lifecycle.committed_open() || entry.lifecycle.pending_open() == Some(open)
        {
            return Ok(OpenChangeDispatch::noop(
                id.clone(),
                entry.lease_token,
                entry.generation,
                entry.registration_revision,
            ));
        }
        let ownership = entry.ownership;
        if ownership == OverlayOwnership::Controlled {
            let dispatch = {
                let entry = self
                    .entries
                    .get_mut(id)
                    .expect("overlay existence was checked");
                let pending = entry
                    .lifecycle
                    .request_controlled(open, reason)
                    .expect("controlled request was checked for duplication");
                entry.open_change_revision = entry.open_change_revision.wrapping_add(1);
                OpenChangeDispatch {
                    layer_id: id.clone(),
                    lease_token: entry.lease_token,
                    generation: entry.generation,
                    registration_revision: entry.registration_revision,
                    open_change_revision: entry.open_change_revision,
                    ownership,
                    focus_transition: FocusTransition::None,
                    uncontrolled_commit: None,
                    notify_open_change: entry.on_open_change.is_some(),
                    intent: Some(OpenIntentDispatch {
                        desired_open: open,
                        reason,
                        revision: Some(pending.revision),
                    }),
                    changed: true,
                }
            };
            return Ok(dispatch);
        }

        let old_phase = self.entries[id].lifecycle.phase();
        let next_phase = if open {
            OverlayLayerPhase::Open
        } else {
            OverlayLayerPhase::Closing
        };
        if open {
            self.validate_parent_lifecycle(self.entries[id].parent.as_ref(), next_phase)?;
        } else if self.has_present_descendants(id) {
            return Err(WindowOverlayRuntimeError::PresentDescendants(id.clone()));
        }
        let focus_transition = self.lifecycle_transition(id, old_phase, next_phase);
        let mouse_authority_changed = {
            let entry = &self.entries[id];
            MouseAuthorityProfile::from_policy(&entry.policy, old_phase, entry.presentation)
                != MouseAuthorityProfile::from_policy(&entry.policy, next_phase, entry.presentation)
        };
        let entry = self
            .entries
            .get_mut(id)
            .expect("overlay existence was checked");
        entry.open_change_revision = entry.open_change_revision.wrapping_add(1);
        if old_phase != next_phase {
            entry.generation = OverlayLayerGeneration(entry.generation.0.wrapping_add(1));
        }
        if open {
            entry.lifecycle.rebind_presence(OverlayPresence::open());
        } else {
            entry
                .lifecycle
                .transition_to_noninteractive(OverlayLayerPhase::Closing, None);
        }
        if open {
            entry.focus_entered = false;
            entry.forced_by_ancestor = false;
        }
        entry.focus_active = matches!(&focus_transition, FocusTransition::Activate(_))
            || (entry.focus_active
                && !matches!(&focus_transition, FocusTransition::Deactivate { .. }));
        let generation = entry.generation;
        let registration_revision = entry.registration_revision;
        let open_change_revision = entry.open_change_revision;
        let lease_token = entry.lease_token;
        let uncontrolled_commit = entry.uncontrolled_commit.clone();
        let notify_open_change = entry.on_open_change.is_some();
        self.sync_stack(id, old_phase, next_phase, open);
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        Ok(OpenChangeDispatch {
            layer_id: id.clone(),
            lease_token,
            generation,
            registration_revision,
            open_change_revision,
            ownership,
            focus_transition,
            uncontrolled_commit,
            notify_open_change,
            intent: Some(OpenIntentDispatch {
                desired_open: open,
                reason,
                revision: None,
            }),
            changed: true,
        })
    }

    pub(super) fn force_close_for_ancestor(
        &mut self,
        id: &OverlayLayerId,
        reason: DismissReason,
        allow_restore: bool,
        next_phase: OverlayLayerPhase,
        forced_by_ancestor: bool,
    ) -> Result<OpenChangeDispatch, WindowOverlayRuntimeError> {
        debug_assert!(matches!(
            next_phase,
            OverlayLayerPhase::Closing | OverlayLayerPhase::Hidden
        ));
        self.validate_request_target(id, None)?;
        let entry = &self.entries[id];
        let owner_was_open = entry.lifecycle.committed_open();
        if entry.lifecycle.phase() == next_phase
            || (!owner_was_open && next_phase == OverlayLayerPhase::Closing)
        {
            let lease_token = entry.lease_token;
            let generation = entry.generation;
            let registration_revision = entry.registration_revision;
            if forced_by_ancestor {
                self.entries
                    .get_mut(id)
                    .expect("overlay existence was checked before ancestor close")
                    .forced_by_ancestor = true;
            }
            return Ok(OpenChangeDispatch::noop(
                id.clone(),
                lease_token,
                generation,
                registration_revision,
            ));
        }

        let ownership = entry.ownership;
        let intent_already_pending = entry.lifecycle.pending_open() == Some(false);
        let old_phase = entry.lifecycle.phase();
        let existing_pending = entry.lifecycle.pending();
        let mouse_authority_changed =
            MouseAuthorityProfile::from_policy(&entry.policy, old_phase, entry.presentation)
                != MouseAuthorityProfile::from_policy(
                    &entry.policy,
                    next_phase,
                    entry.presentation,
                );
        let mut focus_transition = self.lifecycle_transition(id, old_phase, next_phase);
        if !allow_restore && let FocusTransition::Deactivate { restore, .. } = &mut focus_transition
        {
            *restore = false;
        }

        let entry = self
            .entries
            .get_mut(id)
            .expect("overlay existence was checked before ancestor close");
        if !intent_already_pending {
            entry.open_change_revision = entry.open_change_revision.wrapping_add(1);
        }
        if old_phase != next_phase {
            entry.generation = OverlayLayerGeneration(entry.generation.0.wrapping_add(1));
        }
        entry.focus_active = false;
        entry.forced_by_ancestor = forced_by_ancestor;
        let intent_revision = if owner_was_open && ownership == OverlayOwnership::Controlled {
            Some(
                entry
                    .lifecycle
                    .force_controlled_close(next_phase, reason)
                    .revision,
            )
        } else if ownership == OverlayOwnership::Uncontrolled {
            entry
                .lifecycle
                .transition_to_noninteractive(next_phase, None);
            None
        } else {
            entry
                .lifecycle
                .transition_to_noninteractive(next_phase, existing_pending);
            existing_pending.map(|pending| pending.revision)
        };
        let generation = entry.generation;
        let registration_revision = entry.registration_revision;
        let open_change_revision = entry.open_change_revision;
        let lease_token = entry.lease_token;
        let uncontrolled_commit = (owner_was_open && ownership == OverlayOwnership::Uncontrolled)
            .then(|| entry.uncontrolled_commit.clone())
            .flatten();
        let notify_open_change =
            owner_was_open && !intent_already_pending && entry.on_open_change.is_some();
        self.sync_stack(id, old_phase, next_phase, false);
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        Ok(OpenChangeDispatch {
            layer_id: id.clone(),
            lease_token,
            generation,
            registration_revision,
            open_change_revision,
            ownership,
            focus_transition,
            uncontrolled_commit,
            notify_open_change,
            intent: Some(OpenIntentDispatch {
                desired_open: false,
                reason,
                revision: intent_revision,
            }),
            changed: true,
        })
    }

    pub(super) fn finish_exit(
        &mut self,
        lease: &OverlayLayerLease,
        generation: OverlayLayerGeneration,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_mutable_lease(lease)?;
        let entry = self
            .entries
            .get(&lease.layer_id)
            .expect("overlay lease was validated before finishing exit");
        if entry.generation != generation || entry.lifecycle.phase() != OverlayLayerPhase::Closing {
            return Err(WindowOverlayRuntimeError::StaleGeneration(
                lease.layer_id.clone(),
            ));
        }
        self.finalize_forced_descendants(&lease.layer_id)?;
        let mouse_authority_changed = {
            let entry = &self.entries[&lease.layer_id];
            MouseAuthorityProfile::from_policy(
                &entry.policy,
                OverlayLayerPhase::Closing,
                entry.presentation,
            ) != MouseAuthorityProfile::from_policy(
                &entry.policy,
                OverlayLayerPhase::Hidden,
                entry.presentation,
            )
        };
        let entry = self
            .entries
            .get_mut(&lease.layer_id)
            .expect("overlay lease was validated before finishing exit");
        entry.lifecycle.finish_exit();
        self.stack.retain(|candidate| candidate != &lease.layer_id);
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        Ok(())
    }

    pub(super) fn reject_controlled_intent(
        &mut self,
        lease: &OverlayLayerLease,
        revision: OverlayOpenIntentRevision,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_mutable_lease(lease)?;
        let entry = self
            .entries
            .get_mut(&lease.layer_id)
            .expect("overlay lease was validated before resolving its intent");
        if entry.ownership != OverlayOwnership::Controlled {
            return Err(WindowOverlayRuntimeError::NotControlled(
                lease.layer_id.clone(),
            ));
        }
        if entry.lifecycle.reject_close_intent(revision).is_err() {
            return Err(WindowOverlayRuntimeError::StaleIntent(
                lease.layer_id.clone(),
            ));
        }
        entry.registration_revision = entry.registration_revision.wrapping_add(1);
        Ok(())
    }

    pub(super) fn begin_unregister(
        &mut self,
        lease: &OverlayLayerLease,
    ) -> Result<FocusTransition, WindowOverlayRuntimeError> {
        self.validate_mutable_lease(lease)?;
        if self.has_registered_children(&lease.layer_id) {
            return Err(WindowOverlayRuntimeError::HasChildren(
                lease.layer_id.clone(),
            ));
        }
        Ok(self.mark_unregistering(&lease.layer_id, true))
    }

    pub(super) fn begin_unregister_subtree(
        &mut self,
        lease: &OverlayLayerLease,
    ) -> Result<SubtreeUnregisterPlan, WindowOverlayRuntimeError> {
        self.validate_lease(lease)?;
        let mut layer_ids = self
            .registration_order
            .iter()
            .filter(|candidate| self.is_descendant_or_same(&lease.layer_id, candidate))
            .cloned()
            .collect::<Vec<_>>();
        layer_ids.sort_by(|left, right| {
            self.teardown_order_key(right)
                .cmp(&self.teardown_order_key(left))
        });

        let restore_owner = self.subtree_restore_owner(&lease.layer_id, &layer_ids);
        let cancel_focus_claims =
            self.subtree_focus_claim_cancellations(&layer_ids, restore_owner.as_ref());

        let removals = layer_ids
            .into_iter()
            .map(|layer_id| {
                let allow_restore = restore_owner.as_ref() == Some(&layer_id);
                let transition = self.mark_unregistering(&layer_id, allow_restore);
                let entry = self
                    .entries
                    .get(&layer_id)
                    .expect("release subtree was collected from registered layers");
                (
                    OverlayLayerLease {
                        layer_id,
                        token: entry.lease_token,
                        window_id: lease.window_id,
                    },
                    transition,
                )
            })
            .collect();
        Ok(SubtreeUnregisterPlan {
            cancel_focus_claims,
            removals,
        })
    }

    pub(super) fn record_component_bind(
        &mut self,
        lease: &OverlayLayerLease,
        frame_revision: u64,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let entry = self.entry_for_lease_mut(lease)?;
        entry
            .inside_regions
            .retain(|_, region| region.valid_through >= frame_revision);
        entry.component_bind_revision = Some(frame_revision);
        Ok(())
    }

    pub(super) fn begin_stale_component_subtree_replacement(
        &mut self,
        root: &OverlayLayerId,
        window_id: WindowId,
        frame_revision: u64,
        owner_id: EntityId,
    ) -> Option<SubtreeUnregisterPlan> {
        if !self.entries.get(root).is_some_and(|entry| {
            entry.owner_entity.as_ref().is_some_and(|owner| {
                owner.entity_id() != owner_id
                    && (!owner.is_upgradable()
                        || (entry
                            .component_bind_revision
                            .is_some_and(|revision| revision != frame_revision)
                            && entry
                                .inside_regions
                                .values()
                                .all(|region| region.valid_through != frame_revision)))
            })
        }) {
            return None;
        }

        let mut layer_ids = self
            .registration_order
            .iter()
            .filter(|candidate| self.is_descendant_or_same(root, candidate))
            .cloned()
            .collect::<Vec<_>>();
        layer_ids.sort_by(|left, right| {
            self.teardown_order_key(right)
                .cmp(&self.teardown_order_key(left))
        });
        let cancel_focus_claims = self.subtree_focus_claim_cancellations(&layer_ids, None);
        let removals = layer_ids
            .into_iter()
            .map(|layer_id| {
                let transition = self.mark_unregistering(&layer_id, false);
                let entry = self
                    .entries
                    .get(&layer_id)
                    .expect("replacement subtree was collected from registered layers");
                (
                    OverlayLayerLease {
                        layer_id,
                        token: entry.lease_token,
                        window_id,
                    },
                    transition,
                )
            })
            .collect();
        Some(SubtreeUnregisterPlan {
            cancel_focus_claims,
            removals,
        })
    }

    pub(super) fn mark_unregistering(
        &mut self,
        layer_id: &OverlayLayerId,
        allow_restore: bool,
    ) -> FocusTransition {
        let (transition, mouse_authority_changed) = {
            let entry = self
                .entries
                .get_mut(layer_id)
                .expect("overlay lease was validated before unregister");
            if entry.pending_unregister {
                return FocusTransition::None;
            }
            let mouse_authority_changed = MouseAuthorityProfile::from_policy(
                &entry.policy,
                entry.lifecycle.phase(),
                entry.presentation,
            ) != MouseAuthorityProfile::from_policy(
                &entry.policy,
                OverlayLayerPhase::Hidden,
                entry.presentation,
            );
            let transition = if entry.focus_active {
                let restore = allow_restore && entry.should_restore_focus();
                entry.focus_active = false;
                entry
                    .scope_id
                    .clone()
                    .map(|scope| FocusTransition::Deactivate { scope, restore })
                    .unwrap_or(FocusTransition::None)
            } else {
                FocusTransition::None
            };
            let pending = entry.lifecycle.pending();
            entry
                .lifecycle
                .transition_to_noninteractive(OverlayLayerPhase::Hidden, pending);
            entry.pending_unregister = true;
            (transition, mouse_authority_changed)
        };
        self.stack.retain(|candidate| candidate != layer_id);
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        transition
    }

    pub(super) fn take_unregistered(&mut self, lease: &OverlayLayerLease) -> Option<FocusCleanup> {
        self.pending_unregister_cleanup(lease)?;
        let entry = self.entries.remove(&lease.layer_id)?;
        self.registration_order
            .retain(|candidate| candidate != &lease.layer_id);
        Some(FocusCleanup {
            scope_id: entry.scope_id,
            trigger_id: entry.trigger_id,
        })
    }

    pub(super) fn pending_unregister_cleanup(
        &self,
        lease: &OverlayLayerLease,
    ) -> Option<FocusCleanup> {
        let entry = self
            .entries
            .get(&lease.layer_id)
            .filter(|entry| entry.lease_token == lease.token && entry.pending_unregister)?;
        Some(FocusCleanup {
            scope_id: entry.scope_id.clone(),
            trigger_id: entry.trigger_id.clone(),
        })
    }

    pub(super) fn pending_unregister_parent_lease(
        &self,
        lease: &OverlayLayerLease,
    ) -> Option<OverlayLayerLease> {
        let entry = self
            .entries
            .get(&lease.layer_id)
            .filter(|entry| entry.lease_token == lease.token && entry.pending_unregister)?;
        let parent_id = entry.parent.as_ref()?;
        let parent = self
            .entries
            .get(parent_id)
            .filter(|parent| parent.pending_unregister)?;
        Some(OverlayLayerLease {
            layer_id: parent_id.clone(),
            token: parent.lease_token,
            window_id: lease.window_id,
        })
    }

    pub(super) fn has_registered_children(&self, layer_id: &OverlayLayerId) -> bool {
        self.entries
            .values()
            .any(|entry| entry.parent.as_ref() == Some(layer_id))
    }

    pub(super) fn remove_layer_without_focus(&mut self, id: &OverlayLayerId, lease_token: u64) {
        if let Some(entry) = self
            .entries
            .get(id)
            .filter(|entry| entry.lease_token == lease_token)
        {
            let mouse_authority_changed = MouseAuthorityProfile::from_policy(
                &entry.policy,
                entry.lifecycle.phase(),
                entry.presentation,
            )
            .affects_routing();
            self.entries.remove(id);
            self.registration_order.retain(|candidate| candidate != id);
            self.stack.retain(|candidate| candidate != id);
            if mouse_authority_changed {
                self.bump_mouse_authority();
            }
        }
    }

    pub(super) fn entry_for_lease_mut(
        &mut self,
        lease: &OverlayLayerLease,
    ) -> Result<&mut LayerEntry, WindowOverlayRuntimeError> {
        self.validate_mutable_lease(lease)?;
        Ok(self
            .entries
            .get_mut(&lease.layer_id)
            .expect("overlay lease was validated before mutation"))
    }

    pub(super) fn validate_lease(
        &self,
        lease: &OverlayLayerLease,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let Some(entry) = self.entries.get(&lease.layer_id) else {
            return Err(WindowOverlayRuntimeError::UnknownLayer(
                lease.layer_id.clone(),
            ));
        };
        if entry.lease_token != lease.token {
            return Err(WindowOverlayRuntimeError::ForeignLease(
                lease.layer_id.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn lease_status(&self, lease: &OverlayLayerLease) -> OverlayLayerLeaseStatus {
        let Some(entry) = self
            .entries
            .get(&lease.layer_id)
            .filter(|entry| entry.lease_token == lease.token)
        else {
            return OverlayLayerLeaseStatus::Released;
        };
        if entry.pending_unregister {
            OverlayLayerLeaseStatus::PendingUnregister
        } else {
            OverlayLayerLeaseStatus::Registered {
                phase: entry.lifecycle.phase(),
            }
        }
    }

    pub(super) fn validate_mutable_lease(
        &self,
        lease: &OverlayLayerLease,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.validate_lease(lease)?;
        if self.entries[&lease.layer_id].pending_unregister {
            return Err(WindowOverlayRuntimeError::LayerUnregistering(
                lease.layer_id.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn validate_parent(
        &self,
        layer_id: &OverlayLayerId,
        parent: Option<&OverlayLayerId>,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let Some(parent) = parent else {
            return Ok(());
        };
        let parent_entry = self
            .entries
            .get(parent)
            .ok_or_else(|| WindowOverlayRuntimeError::MissingParent(parent.clone()))?;
        if parent_entry.pending_unregister {
            return Err(WindowOverlayRuntimeError::LayerUnregistering(
                parent.clone(),
            ));
        }
        if self.is_descendant_or_same(layer_id, parent) {
            return Err(WindowOverlayRuntimeError::CyclicParent(layer_id.clone()));
        }
        Ok(())
    }

    pub(super) fn validate_parent_lifecycle(
        &self,
        parent: Option<&OverlayLayerId>,
        child_phase: OverlayLayerPhase,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let mut current = parent;
        while let Some(parent_id) = current {
            let entry = self
                .entries
                .get(parent_id)
                .ok_or_else(|| WindowOverlayRuntimeError::MissingParent(parent_id.clone()))?;
            let compatible = match child_phase {
                OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested => {
                    entry.lifecycle.committed_open()
                }
                OverlayLayerPhase::Closing => entry.lifecycle.presence().present(),
                OverlayLayerPhase::Hidden => true,
            };
            if !compatible {
                return Err(WindowOverlayRuntimeError::InactiveAncestor(
                    parent_id.clone(),
                ));
            }
            current = entry.parent.as_ref();
        }
        Ok(())
    }

    pub(super) fn has_present_descendants(&self, layer_id: &OverlayLayerId) -> bool {
        self.entries.values().any(|entry| {
            entry.id != *layer_id
                && self.is_descendant_or_same(layer_id, &entry.id)
                && entry.lifecycle.presence().present()
        })
    }

    pub(super) fn dispatch_is_current(
        &self,
        layer_id: &OverlayLayerId,
        lease_token: u64,
        generation: OverlayLayerGeneration,
        registration_revision: u64,
    ) -> bool {
        self.entries.get(layer_id).is_some_and(|entry| {
            entry.lease_token == lease_token
                && entry.generation == generation
                && entry.registration_revision == registration_revision
                && !entry.pending_unregister
        })
    }

    pub(super) fn current_open_change_callback(
        &self,
        layer_id: &OverlayLayerId,
        lease_token: u64,
        open_change_revision: u64,
        ownership: OverlayOwnership,
        intent: OpenIntentDispatch,
    ) -> Option<OpenChangeCallback> {
        let entry = self.entries.get(layer_id)?;
        if entry.lease_token != lease_token
            || entry.open_change_revision != open_change_revision
            || entry.ownership != ownership
            || entry.pending_unregister
        {
            return None;
        }

        match (ownership, intent.revision) {
            (OverlayOwnership::Controlled, Some(revision)) => {
                let pending = entry.lifecycle.pending()?;
                if pending.open != intent.desired_open
                    || pending.reason != intent.reason
                    || pending.revision != revision
                {
                    return None;
                }
            }
            (OverlayOwnership::Uncontrolled, None) => {}
            _ => return None,
        }

        entry.on_open_change.clone()
    }

    pub(super) fn finalize_forced_descendants(
        &mut self,
        layer_id: &OverlayLayerId,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let mut descendants = self
            .registration_order
            .iter()
            .filter(|candidate| {
                **candidate != *layer_id
                    && self.is_descendant_or_same(layer_id, candidate)
                    && self.entries[*candidate].lifecycle.presence().present()
            })
            .cloned()
            .collect::<Vec<_>>();
        if descendants
            .iter()
            .any(|descendant| !self.entries[descendant].forced_by_ancestor)
        {
            return Err(WindowOverlayRuntimeError::PresentDescendants(
                layer_id.clone(),
            ));
        }
        descendants.sort_by(|left, right| {
            self.teardown_order_key(right)
                .cmp(&self.teardown_order_key(left))
        });
        let mouse_authority_changed = descendants.iter().any(|descendant| {
            let entry = &self.entries[descendant];
            MouseAuthorityProfile::from_policy(
                &entry.policy,
                entry.lifecycle.phase(),
                entry.presentation,
            ) != MouseAuthorityProfile::from_policy(
                &entry.policy,
                OverlayLayerPhase::Hidden,
                entry.presentation,
            )
        });
        for descendant in descendants {
            let entry = self
                .entries
                .get_mut(&descendant)
                .expect("forced descendant was collected from registered layers");
            entry.lifecycle.finish_exit();
            self.stack.retain(|candidate| candidate != &descendant);
        }
        if mouse_authority_changed {
            self.bump_mouse_authority();
        }
        Ok(())
    }

    pub(super) fn sync_stack(
        &mut self,
        id: &OverlayLayerId,
        old_phase: OverlayLayerPhase,
        next_phase: OverlayLayerPhase,
        move_to_top: bool,
    ) {
        if next_phase == OverlayLayerPhase::Hidden {
            self.stack.retain(|candidate| candidate != id);
            return;
        }
        if old_phase == OverlayLayerPhase::Hidden || move_to_top {
            let mut subtree = self
                .stack
                .iter()
                .filter(|candidate| self.is_descendant_or_same(id, candidate))
                .cloned()
                .collect::<Vec<_>>();
            if !subtree.contains(id) {
                subtree.insert(0, id.clone());
            }
            let subtree_ids = subtree.iter().collect::<HashSet<_>>();
            self.stack
                .retain(|candidate| !subtree_ids.contains(candidate));
            self.stack.extend(subtree);
        }
    }

    pub(super) fn is_descendant_or_same(
        &self,
        ancestor: &OverlayLayerId,
        candidate: &OverlayLayerId,
    ) -> bool {
        let mut current = Some(candidate);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.entries.get(id).and_then(|entry| entry.parent.as_ref());
        }
        false
    }

    pub(super) fn layer_depth(&self, layer_id: &OverlayLayerId) -> usize {
        let mut depth = 0;
        let mut current = self
            .entries
            .get(layer_id)
            .and_then(|entry| entry.parent.as_ref());
        while let Some(parent) = current {
            depth += 1;
            current = self
                .entries
                .get(parent)
                .and_then(|entry| entry.parent.as_ref());
        }
        depth
    }

    pub(super) fn teardown_order_key(&self, layer_id: &OverlayLayerId) -> (usize, bool, usize) {
        let stack_index = self
            .stack
            .iter()
            .position(|candidate| candidate == layer_id);
        let stable_order = stack_index.unwrap_or_else(|| {
            self.registration_order
                .iter()
                .position(|candidate| candidate == layer_id)
                .unwrap_or(0)
        });
        (
            self.layer_depth(layer_id),
            stack_index.is_some(),
            stable_order,
        )
    }

    pub(super) fn snapshot(&self, window_id: WindowId) -> WindowOverlaySnapshot {
        let mut ids = self.stack.clone();
        let hidden_ids = self
            .registration_order
            .iter()
            .filter(|id| !ids.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        ids.extend(hidden_ids);
        let layers = ids
            .into_iter()
            .filter_map(|id| self.entries.get(&id))
            .map(LayerEntry::snapshot)
            .collect();
        WindowOverlaySnapshot { window_id, layers }
    }
}
