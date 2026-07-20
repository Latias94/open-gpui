//! GPUI adapter for renderer-neutral focus scope policy.

use std::collections::BTreeMap;
use std::fmt;

use open_gpui::{
    App, AppContext as _, Entity, FocusHandle, KeyDownEvent, WeakFocusHandle, Window, WindowId,
};
use open_gpui_ui_core::{
    FocusResolution, FocusRestoreInput, FocusRestoreIntent, FocusScopeId, FocusScopeMode,
    FocusScopePolicy, FocusTargetAvailability, FocusTargetCandidate, FocusTargetId,
    InitialFocusIntent, resolve_focus_scope_restore,
};

/// Failure returned while configuring or driving a GPUI focus scope runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusScopeRuntimeError {
    /// The runtime was used with a window other than the one that created it.
    WrongWindow,
    /// A scope with this identity is already registered.
    DuplicateScope(FocusScopeId),
    /// A scope references a parent that is not registered in this window.
    MissingParent(FocusScopeId),
    /// A requested scope is not registered.
    UnknownScope(FocusScopeId),
    /// Rebinding a scope would make its parent chain cyclic.
    CyclicScope(FocusScopeId),
    /// A target with this identity is already registered in this window.
    DuplicateTarget(FocusTargetId),
    /// A live handle is already owned by another canonical logical target.
    DuplicateTargetHandle(FocusTargetId),
    /// A requested target is not registered in this window.
    UnknownTarget(FocusTargetId),
    /// A target references a scope that is not registered.
    MissingTargetScope(FocusScopeId),
    /// A target belongs to a different scope than the batch being synchronized.
    TargetScopeMismatch {
        /// Target whose ownership does not match the batch scope.
        target: FocusTargetId,
        /// Scope owned by the batch operation.
        expected: FocusScopeId,
        /// Scope currently declared by the target, or `None` for an unscoped target.
        actual: Option<FocusScopeId>,
    },
    /// A window application fallback must not be owned by a focus scope.
    ScopedWindowFallback(FocusTargetId),
}

impl fmt::Display for FocusScopeRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWindow => {
                formatter.write_str("focus scope runtime belongs to another window")
            }
            Self::DuplicateScope(scope) => write!(formatter, "duplicate focus scope `{scope}`"),
            Self::MissingParent(scope) => {
                write!(formatter, "focus scope parent `{scope}` is not registered")
            }
            Self::UnknownScope(scope) => write!(formatter, "unknown focus scope `{scope}`"),
            Self::CyclicScope(scope) => write!(formatter, "cyclic focus scope `{scope}`"),
            Self::DuplicateTarget(target) => write!(formatter, "duplicate focus target `{target}`"),
            Self::DuplicateTargetHandle(target) => {
                write!(
                    formatter,
                    "focus handle is already registered as `{target}`"
                )
            }
            Self::UnknownTarget(target) => write!(formatter, "unknown focus target `{target}`"),
            Self::MissingTargetScope(scope) => {
                write!(formatter, "focus target scope `{scope}` is not registered")
            }
            Self::TargetScopeMismatch {
                target,
                expected,
                actual,
            } => match actual {
                Some(actual) => write!(
                    formatter,
                    "focus target `{target}` belongs to scope `{actual}`, expected `{expected}`"
                ),
                None => write!(
                    formatter,
                    "focus target `{target}` is unscoped, expected scope `{expected}`"
                ),
            },
            Self::ScopedWindowFallback(target) => {
                write!(
                    formatter,
                    "window fallback `{target}` belongs to a focus scope"
                )
            }
        }
    }
}

impl std::error::Error for FocusScopeRuntimeError {}

/// Registration for one rendered GPUI focus scope root.
#[derive(Debug, Clone)]
pub(crate) struct FocusScopeRegistration {
    policy: FocusScopePolicy,
    root: WeakFocusHandle,
    trigger: Option<FocusTargetId>,
    surface: Option<FocusTargetId>,
}

impl FocusScopeRegistration {
    /// Creates a scope registration from renderer-neutral policy and a rendered root handle.
    pub(crate) fn new(policy: FocusScopePolicy, root: &FocusHandle) -> Self {
        Self {
            policy,
            root: root.downgrade(),
            trigger: None,
            surface: None,
        }
    }

    /// Registers the logical target that opened this scope.
    pub(crate) fn with_trigger(mut self, trigger: impl Into<FocusTargetId>) -> Self {
        self.trigger = Some(trigger.into());
        self
    }

    /// Registers a named target as the non-tab surface fallback.
    pub(crate) fn with_surface(mut self, surface: impl Into<FocusTargetId>) -> Self {
        self.surface = Some(surface.into());
        self
    }
}

/// Registration for one named GPUI focus target.
#[derive(Debug, Clone)]
pub struct FocusTargetRegistration {
    id: FocusTargetId,
    scope: Option<FocusScopeId>,
    handle: WeakFocusHandle,
    availability: FocusTargetAvailability,
}

impl FocusTargetRegistration {
    /// Creates an available target outside any focus scope.
    pub fn new(id: impl Into<FocusTargetId>, handle: &FocusHandle) -> Self {
        Self {
            id: id.into(),
            scope: None,
            handle: handle.downgrade(),
            availability: FocusTargetAvailability::Available,
        }
    }

    /// Associates this runtime-owned target with a focus scope.
    pub(crate) fn within_scope(mut self, scope: impl Into<FocusScopeId>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Applies renderer-projected availability for this target.
    pub fn with_availability(mut self, availability: FocusTargetAvailability) -> Self {
        self.availability = availability;
        self
    }

    /// Returns the stable target identity.
    pub const fn id(&self) -> &FocusTargetId {
        &self.id
    }

    pub(crate) fn assigned_to_scope(mut self, scope: Option<FocusScopeId>) -> Self {
        self.scope = scope;
        self
    }

    pub(crate) fn assigned_id(mut self, id: FocusTargetId) -> Self {
        self.id = id;
        self
    }
}

/// A focus scope runtime created and owned by one GPUI window.
///
/// This is a low-level adapter seam. The window overlay runtime owns its production instance;
/// component families must not create app-global registries or independent scope runtimes.
#[derive(Clone)]
pub(crate) struct FocusScopeRuntime {
    state: Entity<FocusScopeRuntimeState>,
    window_id: WindowId,
}

impl FocusScopeRuntime {
    /// Creates a focus scope runtime bound to the current window.
    pub(crate) fn new(window: &Window, cx: &mut App) -> Self {
        let window_id = window.window_handle().window_id();
        let state = cx.new(|_| FocusScopeRuntimeState::default());
        Self { state, window_id }
    }

    /// Registers a focus scope.
    #[cfg(test)]
    pub(crate) fn register_scope(
        &self,
        registration: FocusScopeRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.register_scope(registration))
    }

    pub(crate) fn register_scope_bundle(
        &self,
        trigger: FocusTargetRegistration,
        scope: FocusScopeRegistration,
        surface: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.register_scope_bundle(trigger, scope, surface)
        })
    }

    /// Rebinds an existing logical scope to its latest policy and live root handle.
    #[cfg(test)]
    pub(crate) fn rebind_scope(
        &self,
        registration: FocusScopeRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.rebind_scope(registration))
    }

    pub(crate) fn rebind_scope_bundle(
        &self,
        trigger: FocusTargetRegistration,
        scope: FocusScopeRegistration,
        surface: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.rebind_scope_bundle(trigger, scope, surface)
        })
    }

    pub(crate) fn unregister_scope_bundle(
        &self,
        trigger: &FocusTargetId,
        scope: &FocusScopeId,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.unregister_scope_bundle(trigger, scope))
    }

    /// Removes a logical scope, its descendants, and their registered targets.
    #[cfg(test)]
    pub(crate) fn unregister_scope(
        &self,
        scope: &FocusScopeId,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.unregister_scope(scope))
    }

    /// Registers a stable logical target and its live GPUI handle.
    pub(crate) fn register_target(
        &self,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.register_target(registration))
    }

    /// Rebinds an existing logical target to its latest live handle and availability.
    ///
    /// The owning overlay runtime supplies the canonical scope assignment.
    pub(crate) fn rebind_target(
        &self,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.rebind_target(registration))
    }

    /// Atomically replaces one scope's caller-managed targets and re-applies initial focus when
    /// the currently focused logical target is removed, rebound, or becomes unavailable.
    pub(crate) fn sync_targets(
        &self,
        scope: &FocusScopeId,
        previous_targets: &[FocusTargetId],
        registrations: Vec<FocusTargetRegistration>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        let current = window.focused(cx);
        let validation = self.state.update(cx, |state, _| {
            state.sync_targets(
                scope,
                previous_targets,
                registrations,
                current.as_ref(),
                window,
            )
        })?;
        if let Some(validation) = validation {
            self.schedule_target_validation(validation, window, cx);
        }
        Ok(())
    }

    /// Removes a logical target and clears runtime references to it.
    pub(crate) fn unregister_target(
        &self,
        target: &FocusTargetId,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.unregister_target(target))
    }

    /// Updates renderer-projected target availability without replacing its stable identity.
    #[cfg(test)]
    pub(crate) fn set_target_availability(
        &self,
        target: &FocusTargetId,
        availability: FocusTargetAvailability,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            let Some(target_entry) = state.targets.get_mut(target) else {
                return Err(FocusScopeRuntimeError::UnknownTarget(target.clone()));
            };
            target_entry.availability = availability;
            Ok(())
        })
    }

    /// Registers or clears the application fallback target for this window.
    #[cfg(test)]
    pub(crate) fn set_window_fallback(
        &self,
        target: Option<FocusTargetId>,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.set_window_fallback(target))
    }

    pub(crate) fn register_window_fallback_target(
        &self,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.register_window_fallback_target(registration)
        })
    }

    pub(crate) fn rebind_window_fallback_target(
        &self,
        registration: FocusTargetRegistration,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.rebind_window_fallback_target(registration)
        })
    }

    pub(crate) fn unregister_window_fallback_target(
        &self,
        target: &FocusTargetId,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            state.unregister_window_fallback_target(target)
        })
    }

    /// Activates a scope, captures the current logical target, and queues initial focus.
    pub(crate) fn activate_scope(
        &self,
        scope: FocusScopeId,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        let current = window.focused(cx);
        let claim = self.state.update(cx, |state, _| {
            state.activate_scope(&scope, current.as_ref(), window)
        })?;
        if let Some(claim) = claim {
            self.schedule_claim(claim, window, cx);
        }
        Ok(())
    }

    /// Reactivates a suspended scope without capturing a new restore target or claiming focus.
    pub(crate) fn resume_scope(
        &self,
        scope: FocusScopeId,
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state
            .update(cx, |state, _| state.resume_scope(&scope, window))
    }

    /// Deactivates a committed-open scope and queues deterministic restoration.
    #[cfg(test)]
    pub(crate) fn deactivate_scope(
        &self,
        scope: FocusScopeId,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.deactivate_scope_with_restore(scope, true, window, cx)
    }

    pub(crate) fn deactivate_scope_with_restore(
        &self,
        scope: FocusScopeId,
        restore: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        let current = window.focused(cx);
        let claim = self.state.update(cx, |state, _| {
            state.deactivate_scope(&scope, current.as_ref(), window, restore)
        })?;
        if let Some(claim) = claim {
            self.schedule_claim(claim, window, cx);
        }
        Ok(())
    }

    pub(crate) fn has_pending_claim_for_scope(
        &self,
        scope: &FocusScopeId,
        window: &Window,
        cx: &App,
    ) -> Result<bool, FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        Ok(self.state.read(cx).has_pending_claim_for_scope(scope))
    }

    pub(crate) fn retry_pending_claim_for_scope(
        &self,
        scope: &FocusScopeId,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<PendingClaimRetry, FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        let frame_revision = window.rendered_frame_revision();
        let retry = self.state.update(cx, |state, _| {
            state.schedule_pending_claim_retry(scope, frame_revision)
        });
        let Some(sequence) = retry.sequence else {
            return Ok(retry);
        };
        let runtime = self.clone();
        window.defer(cx, move |window, cx| {
            runtime.commit_claim(sequence, window, cx);
        });
        Ok(retry)
    }

    pub(crate) fn cancel_claims_for_scopes(
        &self,
        scopes: &[FocusScopeId],
        window: &Window,
        cx: &mut App,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.ensure_window(window)?;
        self.state.update(cx, |state, _| {
            for scope in scopes {
                state.cancel_claims_for_scope(scope);
            }
        });
        Ok(())
    }

    /// Handles a real Tab or Shift-Tab event for the innermost live modal scope.
    ///
    /// Returns whether a modal scope consumed the event.
    pub(crate) fn handle_key_down(
        &self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        if self.ensure_window(window).is_err()
            || event.keystroke.key.as_str() != "tab"
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
            || modifiers.function
        {
            return false;
        }

        let traversal = self.state.read(cx).modal_traversal(window);
        let Some(traversal) = traversal else {
            return false;
        };
        let ModalTraversal::Live {
            root,
            unavailable_handles,
            surface,
        } = traversal
        else {
            cx.stop_propagation();
            window.prevent_default();
            return true;
        };

        let is_available = |candidate: &FocusHandle| {
            !unavailable_handles
                .iter()
                .any(|unavailable| unavailable == candidate)
        };
        let mut moved = if modifiers.shift {
            window.focus_prev_where_within(&root, is_available, cx)
        } else {
            window.focus_next_where_within(&root, is_available, cx)
        };
        if !moved && let Some(surface) = surface {
            surface.focus(window, cx);
            moved = true;
        }
        if !moved
            && window
                .focused(cx)
                .is_some_and(|current| !root.contains(&current, window))
        {
            window.blur(cx);
        }
        if moved {
            let current = window.focused(cx);
            self.state.update(cx, |state, _| {
                state.record_focus(current.as_ref(), window);
            });
        }

        cx.stop_propagation();
        window.prevent_default();
        true
    }

    fn ensure_window(&self, window: &Window) -> Result<(), FocusScopeRuntimeError> {
        if window.window_handle().window_id() == self.window_id {
            Ok(())
        } else {
            Err(FocusScopeRuntimeError::WrongWindow)
        }
    }

    fn schedule_claim(&self, kind: PendingFocusClaimKind, window: &mut Window, cx: &mut App) {
        let focus_claim_revision = window.focus_claim_revision();
        let rendered_frame_revision = window.rendered_frame_revision();
        let sequence = self.state.update(cx, |state, _| {
            state.next_claim_sequence = state.next_claim_sequence.wrapping_add(1);
            let sequence = state.next_claim_sequence;
            state.queue_claim(PendingFocusClaim {
                sequence,
                focus_claim_revision,
                rendered_frame_revision,
                kind,
            });
            sequence
        });
        let runtime = self.clone();
        window.defer(cx, move |window, cx| {
            runtime.commit_claim(sequence, window, cx);
        });
    }

    fn schedule_target_validation(
        &self,
        validation: PendingTargetValidationKey,
        window: &mut Window,
        cx: &mut App,
    ) {
        let runtime = self.clone();
        window.defer(cx, move |window, cx| {
            runtime.commit_target_validation(validation, window, cx);
        });
    }

    fn commit_target_validation(
        &self,
        validation: PendingTargetValidationKey,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.ensure_window(window).is_err() {
            return;
        }
        let current = window.focused(cx);
        let commit = self.state.update(cx, |state, _| {
            state.commit_target_validation(&validation, current.as_ref(), window)
        });
        match commit {
            Some(FocusCommit::Focus(target)) => {
                target.focus(window, cx);
                self.state.update(cx, |state, _| {
                    state.record_focus(Some(&target), window);
                });
            }
            Some(FocusCommit::Blur) => window.blur(cx),
            Some(FocusCommit::RetryAfterFrame) => {
                let runtime = self.clone();
                window.on_next_frame(move |window, cx| {
                    runtime.commit_target_validation(validation, window, cx);
                });
                window.refresh();
            }
            Some(FocusCommit::Preserve) | None => {}
        }
    }

    fn commit_claim(&self, sequence: u64, window: &mut Window, cx: &mut App) {
        if self.ensure_window(window).is_err() {
            return;
        }
        let current = window.focused(cx);
        let commit = self.state.update(cx, |state, _| {
            state.clear_scheduled_surface_retry(sequence);
            state.commit_claim(sequence, current.as_ref(), window)
        });
        match commit {
            Some(FocusCommit::Focus(target)) => {
                target.focus(window, cx);
                self.state.update(cx, |state, _| {
                    state.record_focus(Some(&target), window);
                });
            }
            Some(FocusCommit::Blur) => window.blur(cx),
            Some(FocusCommit::RetryAfterFrame) => {
                let runtime = self.clone();
                window.on_next_frame(move |window, cx| {
                    runtime.commit_claim(sequence, window, cx);
                });
                window.refresh();
            }
            Some(FocusCommit::Preserve) | None => {}
        }
    }
}

#[derive(Clone, Default)]
struct FocusScopeRuntimeState {
    scopes: BTreeMap<FocusScopeId, ScopeEntry>,
    targets: BTreeMap<FocusTargetId, TargetEntry>,
    window_fallback: Option<FocusTargetId>,
    next_activation_sequence: u64,
    next_claim_sequence: u64,
    pending_initial_claim: Option<PendingFocusClaim>,
    pending_restore_claim: Option<PendingFocusClaim>,
    scheduled_surface_retries: Vec<ScheduledFocusClaimRetry>,
    next_target_validation_sequence: u64,
    pending_target_validations: BTreeMap<FocusScopeId, PendingTargetValidation>,
}

impl FocusScopeRuntimeState {
    fn atomically(
        &mut self,
        mutate: impl FnOnce(&mut Self) -> Result<(), FocusScopeRuntimeError>,
    ) -> Result<(), FocusScopeRuntimeError> {
        let mut staged = self.clone();
        mutate(&mut staged)?;
        *self = staged;
        Ok(())
    }

    fn queue_claim(&mut self, claim: PendingFocusClaim) {
        self.scheduled_surface_retries
            .retain(|retry| retry.scope != *claim.kind.scope());
        if claim.kind.is_initial() {
            self.pending_initial_claim = Some(claim);
        } else {
            self.pending_restore_claim = Some(claim);
        }
    }

    fn set_window_fallback(
        &mut self,
        target: Option<FocusTargetId>,
    ) -> Result<(), FocusScopeRuntimeError> {
        if let Some(target) = target.as_ref() {
            let Some(entry) = self.targets.get(target) else {
                return Err(FocusScopeRuntimeError::UnknownTarget(target.clone()));
            };
            if entry.scope.is_some() {
                return Err(FocusScopeRuntimeError::ScopedWindowFallback(target.clone()));
            }
        }
        self.window_fallback = target;
        Ok(())
    }

    fn register_window_fallback_target(
        &mut self,
        registration: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        let target_id = registration.id.clone();
        self.atomically(move |state| {
            state.register_target(registration)?;
            state.set_window_fallback(Some(target_id))
        })
    }

    fn rebind_window_fallback_target(
        &mut self,
        registration: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        let target_id = registration.id.clone();
        self.atomically(move |state| {
            state.rebind_target(registration)?;
            state.set_window_fallback(Some(target_id))
        })
    }

    fn unregister_window_fallback_target(
        &mut self,
        target: &FocusTargetId,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.atomically(|state| {
            state.unregister_target(target)?;
            state.set_window_fallback(None)
        })
    }

    fn register_scope(
        &mut self,
        registration: FocusScopeRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        let scope_id = registration.policy.id().clone();
        if self.scopes.contains_key(&scope_id) {
            return Err(FocusScopeRuntimeError::DuplicateScope(scope_id));
        }
        if let Some(parent) = registration.policy.parent()
            && !self.scopes.contains_key(parent)
        {
            return Err(FocusScopeRuntimeError::MissingParent(parent.clone()));
        }
        self.scopes.insert(
            scope_id,
            ScopeEntry {
                policy: registration.policy,
                root: registration.root,
                trigger: registration.trigger,
                surface: registration.surface,
                active: false,
                activation_sequence: 0,
                activation_focus_claim_revision: 0,
                lifecycle_generation: 0,
                saved_target: None,
                last_live_target: None,
            },
        );
        Ok(())
    }

    fn register_scope_bundle(
        &mut self,
        trigger: FocusTargetRegistration,
        scope: FocusScopeRegistration,
        surface: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.atomically(move |state| {
            state.register_target(trigger)?;
            state.register_scope(scope)?;
            state.register_target(surface)
        })
    }

    fn rebind_scope(
        &mut self,
        registration: FocusScopeRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.validate_scope_rebind(&registration)?;
        self.apply_scope_rebind(registration);
        Ok(())
    }

    fn validate_scope_rebind(
        &self,
        registration: &FocusScopeRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        let scope_id = registration.policy.id();
        if !self.scopes.contains_key(&scope_id) {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        }
        if let Some(parent) = registration.policy.parent() {
            if !self.scopes.contains_key(parent) {
                return Err(FocusScopeRuntimeError::MissingParent(parent.clone()));
            }
            if parent == scope_id || self.scope_contains(scope_id, parent) {
                return Err(FocusScopeRuntimeError::CyclicScope(scope_id.clone()));
            }
        }
        Ok(())
    }

    fn apply_scope_rebind(&mut self, registration: FocusScopeRegistration) {
        let scope_id = registration.policy.id().clone();
        let scope = self
            .scopes
            .get_mut(&scope_id)
            .expect("focus scope rebind was validated before commit");
        scope.policy = registration.policy;
        scope.root = registration.root;
        scope.trigger = registration.trigger;
        scope.surface = registration.surface;
    }

    fn rebind_scope_bundle(
        &mut self,
        trigger: FocusTargetRegistration,
        scope: FocusScopeRegistration,
        surface: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        if self.target_matches_registration(&trigger)
            && self.scope_matches_registration(&scope)
            && self.target_matches_registration(&surface)
        {
            return Ok(());
        }
        self.atomically(move |state| {
            state.rebind_target(trigger)?;
            state.rebind_scope(scope)?;
            state.rebind_target(surface)
        })
    }

    fn scope_matches_registration(&self, registration: &FocusScopeRegistration) -> bool {
        self.scopes
            .get(registration.policy.id())
            .is_some_and(|scope| {
                scope.policy == registration.policy
                    && scope.root == registration.root
                    && scope.trigger == registration.trigger
                    && scope.surface == registration.surface
            })
    }

    fn unregister_scope_bundle(
        &mut self,
        trigger: &FocusTargetId,
        scope: &FocusScopeId,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.atomically(|state| {
            state.unregister_target(trigger)?;
            state.unregister_scope(scope)
        })
    }

    fn unregister_scope(&mut self, scope_id: &FocusScopeId) -> Result<(), FocusScopeRuntimeError> {
        if !self.scopes.contains_key(scope_id) {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        }

        let removed_scopes = self
            .scopes
            .keys()
            .filter(|candidate| self.scope_contains(scope_id, candidate))
            .cloned()
            .collect::<Vec<_>>();
        self.scopes
            .retain(|candidate, _| !removed_scopes.contains(candidate));
        self.scheduled_surface_retries
            .retain(|retry| !removed_scopes.contains(&retry.scope));
        self.pending_target_validations
            .retain(|scope, _| !removed_scopes.contains(scope));

        let removed_targets = self
            .targets
            .iter()
            .filter(|(_, target)| {
                target
                    .scope
                    .as_ref()
                    .is_some_and(|scope| removed_scopes.contains(scope))
            })
            .map(|(target, _)| target.clone())
            .collect::<Vec<_>>();
        self.targets
            .retain(|target, _| !removed_targets.contains(target));
        self.clear_target_references(&removed_targets);

        if self
            .pending_initial_claim
            .as_ref()
            .is_some_and(|pending| removed_scopes.contains(pending.kind.scope()))
        {
            self.pending_initial_claim = None;
        }
        if self
            .pending_restore_claim
            .as_ref()
            .is_some_and(|pending| removed_scopes.contains(pending.kind.scope()))
        {
            self.pending_restore_claim = None;
        }
        Ok(())
    }

    fn register_target(
        &mut self,
        registration: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        if self.targets.contains_key(&registration.id) {
            return Err(FocusScopeRuntimeError::DuplicateTarget(registration.id));
        }
        if let Some((target, _)) = self
            .targets
            .iter()
            .find(|(_, target)| target.handle == registration.handle)
        {
            return Err(FocusScopeRuntimeError::DuplicateTargetHandle(
                target.clone(),
            ));
        }
        if let Some(scope) = registration.scope.as_ref()
            && !self.scopes.contains_key(scope)
        {
            return Err(FocusScopeRuntimeError::MissingTargetScope(scope.clone()));
        }
        self.targets.insert(
            registration.id,
            TargetEntry {
                scope: registration.scope,
                handle: registration.handle,
                availability: registration.availability,
            },
        );
        Ok(())
    }

    fn rebind_target(
        &mut self,
        registration: FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        self.validate_target_rebind(&registration)?;
        self.apply_target_rebind(registration);
        Ok(())
    }

    fn sync_targets(
        &mut self,
        scope_id: &FocusScopeId,
        previous_targets: &[FocusTargetId],
        registrations: Vec<FocusTargetRegistration>,
        current: Option<&FocusHandle>,
        window: &Window,
    ) -> Result<Option<PendingTargetValidationKey>, FocusScopeRuntimeError> {
        if !self.scopes.contains_key(scope_id) {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        }
        for target_id in previous_targets {
            let Some(target) = self.targets.get(target_id) else {
                return Err(FocusScopeRuntimeError::UnknownTarget(target_id.clone()));
            };
            if target.scope.as_ref() != Some(scope_id) {
                return Err(FocusScopeRuntimeError::TargetScopeMismatch {
                    target: target_id.clone(),
                    expected: scope_id.clone(),
                    actual: target.scope.clone(),
                });
            }
        }

        let focused_target_before_sync = current.and_then(|current| {
            previous_targets.iter().find_map(|target_id| {
                self.targets
                    .get(target_id)
                    .and_then(|target| target.handle.upgrade())
                    .filter(|handle| handle == current)
                    .map(|_| target_id.clone())
            })
        });
        let mut staged = self.clone();
        for target_id in previous_targets {
            staged.targets.remove(target_id);
        }

        let mut final_target_ids = Vec::with_capacity(registrations.len());
        for registration in registrations {
            if registration.scope.as_ref() != Some(scope_id) {
                return Err(FocusScopeRuntimeError::TargetScopeMismatch {
                    target: registration.id.clone(),
                    expected: scope_id.clone(),
                    actual: registration.scope.clone(),
                });
            }
            final_target_ids.push(registration.id.clone());
            staged.register_target(registration)?;
        }
        let removed_targets = previous_targets
            .iter()
            .filter(|target_id| !final_target_ids.contains(target_id))
            .cloned()
            .collect::<Vec<_>>();
        staged.clear_target_references(&removed_targets);
        let validation = focused_target_before_sync.and_then(|target| {
            current.and_then(|current| {
                staged.queue_target_validation(scope_id, target, current, window)
            })
        });
        *self = staged;
        Ok(validation)
    }

    fn target_matches_registration(&self, registration: &FocusTargetRegistration) -> bool {
        self.targets.get(&registration.id).is_some_and(|target| {
            target.scope == registration.scope
                && target.handle == registration.handle
                && target.availability == registration.availability
        })
    }

    fn validate_target_rebind(
        &self,
        registration: &FocusTargetRegistration,
    ) -> Result<(), FocusScopeRuntimeError> {
        if !self.targets.contains_key(&registration.id) {
            return Err(FocusScopeRuntimeError::UnknownTarget(
                registration.id.clone(),
            ));
        }
        if let Some((target, _)) = self.targets.iter().find(|(target, entry)| {
            *target != &registration.id && entry.handle == registration.handle
        }) {
            return Err(FocusScopeRuntimeError::DuplicateTargetHandle(
                target.clone(),
            ));
        }
        if let Some(scope) = registration.scope.as_ref()
            && !self.scopes.contains_key(scope)
        {
            return Err(FocusScopeRuntimeError::MissingTargetScope(scope.clone()));
        }
        if registration.scope.is_some() && self.window_fallback.as_ref() == Some(&registration.id) {
            return Err(FocusScopeRuntimeError::ScopedWindowFallback(
                registration.id.clone(),
            ));
        }
        Ok(())
    }

    fn apply_target_rebind(&mut self, registration: FocusTargetRegistration) {
        self.targets.insert(
            registration.id,
            TargetEntry {
                scope: registration.scope,
                handle: registration.handle,
                availability: registration.availability,
            },
        );
    }

    fn unregister_target(&mut self, target: &FocusTargetId) -> Result<(), FocusScopeRuntimeError> {
        if self.targets.remove(target).is_none() {
            return Err(FocusScopeRuntimeError::UnknownTarget(target.clone()));
        }
        self.clear_target_references(std::slice::from_ref(target));
        Ok(())
    }

    fn clear_target_references(&mut self, removed_targets: &[FocusTargetId]) {
        if self
            .window_fallback
            .as_ref()
            .is_some_and(|target| removed_targets.contains(target))
        {
            self.window_fallback = None;
        }
        for scope in self.scopes.values_mut() {
            if scope
                .trigger
                .as_ref()
                .is_some_and(|target| removed_targets.contains(target))
            {
                scope.trigger = None;
            }
            if scope
                .saved_target
                .as_ref()
                .is_some_and(|target| removed_targets.contains(target))
            {
                scope.saved_target = None;
            }
            if scope
                .last_live_target
                .as_ref()
                .is_some_and(|target| removed_targets.contains(target))
            {
                scope.last_live_target = None;
            }
        }
    }

    fn activate_scope(
        &mut self,
        scope_id: &FocusScopeId,
        current: Option<&FocusHandle>,
        window: &Window,
    ) -> Result<Option<PendingFocusClaimKind>, FocusScopeRuntimeError> {
        let Some(existing_scope) = self.scopes.get(scope_id) else {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        };
        if existing_scope.active {
            return Ok(None);
        }

        self.cancel_claims_for_scope(scope_id);
        self.record_focus(current, window);
        let saved_target = current.and_then(|current| self.target_id_for_handle(current, window));
        let scope = self
            .scopes
            .get_mut(scope_id)
            .expect("focus scope existence was checked before mutation");
        self.next_activation_sequence = self.next_activation_sequence.wrapping_add(1);
        scope.active = true;
        scope.activation_sequence = self.next_activation_sequence;
        scope.activation_focus_claim_revision = window.focus_claim_revision();
        scope.lifecycle_generation = scope.lifecycle_generation.wrapping_add(1);
        scope.saved_target = saved_target;
        let lifecycle_generation = scope.lifecycle_generation;

        Ok(
            (scope.policy.initial_focus() != &InitialFocusIntent::None).then(|| {
                PendingFocusClaimKind::Initial {
                    scope: scope_id.clone(),
                    lifecycle_generation,
                }
            }),
        )
    }

    fn deactivate_scope(
        &mut self,
        scope_id: &FocusScopeId,
        current: Option<&FocusHandle>,
        window: &Window,
        restore: bool,
    ) -> Result<Option<PendingFocusClaimKind>, FocusScopeRuntimeError> {
        let Some(existing_scope) = self.scopes.get(scope_id) else {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        };
        if !existing_scope.active {
            return Ok(None);
        }

        self.cancel_claims_for_scope(scope_id);
        self.record_focus(current, window);
        let scope = self
            .scopes
            .get_mut(scope_id)
            .expect("focus scope existence was checked before mutation");
        scope.active = false;
        scope.lifecycle_generation = scope.lifecycle_generation.wrapping_add(1);
        let lifecycle_generation = scope.lifecycle_generation;
        Ok(
            (restore && scope.policy.focus_restore() != &FocusRestoreIntent::None).then(|| {
                PendingFocusClaimKind::Restore {
                    scope: scope_id.clone(),
                    lifecycle_generation,
                }
            }),
        )
    }

    fn resume_scope(
        &mut self,
        scope_id: &FocusScopeId,
        window: &Window,
    ) -> Result<(), FocusScopeRuntimeError> {
        let Some(existing_scope) = self.scopes.get(scope_id) else {
            return Err(FocusScopeRuntimeError::UnknownScope(scope_id.clone()));
        };
        if existing_scope.active {
            return Ok(());
        }

        self.cancel_claims_for_scope(scope_id);
        let scope = self
            .scopes
            .get_mut(scope_id)
            .expect("focus scope existence was checked before mutation");
        self.next_activation_sequence = self.next_activation_sequence.wrapping_add(1);
        scope.active = true;
        scope.activation_sequence = self.next_activation_sequence;
        scope.activation_focus_claim_revision = window.focus_claim_revision();
        scope.lifecycle_generation = scope.lifecycle_generation.wrapping_add(1);
        Ok(())
    }

    fn queue_target_validation(
        &mut self,
        scope_id: &FocusScopeId,
        target: FocusTargetId,
        current: &FocusHandle,
        window: &Window,
    ) -> Option<PendingTargetValidationKey> {
        let scope = self.scopes.get(scope_id)?;
        if !scope.active {
            self.pending_target_validations.remove(scope_id);
            return None;
        }
        let lifecycle_generation = scope.lifecycle_generation;
        self.next_target_validation_sequence = self.next_target_validation_sequence.wrapping_add(1);
        let sequence = self.next_target_validation_sequence;
        self.pending_target_validations.insert(
            scope_id.clone(),
            PendingTargetValidation {
                sequence,
                lifecycle_generation,
                target,
                expected_handle: current.downgrade(),
                focus_claim_revision: window.focus_claim_revision(),
                rendered_frame_revision: window.rendered_frame_revision(),
            },
        );
        Some(PendingTargetValidationKey {
            scope: scope_id.clone(),
            sequence,
        })
    }

    fn has_pending_claim_for_scope(&self, scope_id: &FocusScopeId) -> bool {
        self.pending_initial_claim
            .as_ref()
            .is_some_and(|claim| claim.kind.scope() == scope_id)
            || self
                .pending_restore_claim
                .as_ref()
                .is_some_and(|claim| claim.kind.scope() == scope_id)
    }

    fn latest_pending_sequence_for_scope(&self, scope_id: &FocusScopeId) -> Option<u64> {
        self.pending_initial_claim
            .iter()
            .chain(self.pending_restore_claim.iter())
            .filter(|claim| claim.kind.scope() == scope_id)
            .map(|claim| claim.sequence)
            .max()
    }

    fn schedule_pending_claim_retry(
        &mut self,
        scope: &FocusScopeId,
        frame_revision: u64,
    ) -> PendingClaimRetry {
        let Some(sequence) = self.latest_pending_sequence_for_scope(scope) else {
            return PendingClaimRetry::not_pending();
        };
        let retry = ScheduledFocusClaimRetry {
            scope: scope.clone(),
            sequence,
            frame_revision,
        };
        if self.scheduled_surface_retries.contains(&retry) {
            return PendingClaimRetry::already_scheduled();
        }
        self.scheduled_surface_retries.push(retry);
        PendingClaimRetry::scheduled(sequence)
    }

    fn clear_scheduled_surface_retry(&mut self, sequence: u64) {
        self.scheduled_surface_retries
            .retain(|retry| retry.sequence != sequence);
    }

    fn cancel_claims_for_scope(&mut self, scope_id: &FocusScopeId) {
        self.scheduled_surface_retries
            .retain(|retry| &retry.scope != scope_id);
        self.pending_target_validations.remove(scope_id);
        if self
            .pending_initial_claim
            .as_ref()
            .is_some_and(|claim| claim.kind.scope() == scope_id)
        {
            self.pending_initial_claim = None;
        }
        if self
            .pending_restore_claim
            .as_ref()
            .is_some_and(|claim| claim.kind.scope() == scope_id)
        {
            self.pending_restore_claim = None;
        }
    }

    fn modal_traversal(&self, window: &Window) -> Option<ModalTraversal> {
        let (scope_id, scope) = self
            .scopes
            .iter()
            .filter(|(_, scope)| scope.active && scope.policy.mode() == FocusScopeMode::ModalLoop)
            .max_by_key(|(_, scope)| scope.activation_sequence)?;
        let Some(root) = scope.root.upgrade() else {
            return Some(ModalTraversal::Pending);
        };
        if !window.is_focus_handle_rendered(&root) {
            return Some(ModalTraversal::Pending);
        }

        let unavailable_handles = self.unavailable_handles_in_scope(scope_id, window);
        let surface = scope
            .surface
            .as_ref()
            .and_then(|target| self.live_handle_in_scope(target, scope_id, window));

        Some(ModalTraversal::Live {
            root,
            unavailable_handles,
            surface,
        })
    }

    fn commit_claim(
        &mut self,
        sequence: u64,
        current: Option<&FocusHandle>,
        window: &Window,
    ) -> Option<FocusCommit> {
        if self.latest_pending_sequence()? != sequence {
            return None;
        }

        loop {
            if let Some(pending) = self.pending_initial_claim.clone() {
                if pending.focus_claim_revision != window.focus_claim_revision() {
                    self.pending_initial_claim = None;
                    continue;
                }
                let PendingFocusClaimKind::Initial {
                    scope,
                    lifecycle_generation,
                } = pending.kind
                else {
                    unreachable!("the initial claim slot only stores initial claims");
                };
                let Some(scope_entry) = self.scopes.get(&scope) else {
                    self.pending_initial_claim = None;
                    continue;
                };
                if !scope_entry.active || scope_entry.lifecycle_generation != lifecycle_generation {
                    self.pending_initial_claim = None;
                    continue;
                }
                if pending.rendered_frame_revision == window.rendered_frame_revision() {
                    self.retarget_pending_claims(sequence);
                    return Some(FocusCommit::RetryAfterFrame);
                }

                let commit = self
                    .resolve_initial_target(&scope, window)
                    .map_or(FocusCommit::Preserve, FocusCommit::Focus);
                self.pending_initial_claim = None;
                self.pending_restore_claim = None;
                return Some(commit);
            }

            let Some(pending) = self.pending_restore_claim.clone() else {
                return None;
            };
            if pending.focus_claim_revision != window.focus_claim_revision() {
                self.pending_restore_claim = None;
                continue;
            }
            let PendingFocusClaimKind::Restore {
                scope,
                lifecycle_generation,
            } = pending.kind
            else {
                unreachable!("the restore claim slot only stores restore claims");
            };
            let Some(scope_entry) = self.scopes.get(&scope) else {
                self.pending_restore_claim = None;
                continue;
            };
            if scope_entry.active || scope_entry.lifecycle_generation != lifecycle_generation {
                self.pending_restore_claim = None;
                continue;
            }
            if pending.rendered_frame_revision == window.rendered_frame_revision() {
                self.retarget_pending_claims(sequence);
                return Some(FocusCommit::RetryAfterFrame);
            }
            let commit = self.resolve_restore(&scope, current, window);
            self.pending_restore_claim = None;
            return Some(commit);
        }
    }

    fn commit_target_validation(
        &mut self,
        key: &PendingTargetValidationKey,
        current: Option<&FocusHandle>,
        window: &Window,
    ) -> Option<FocusCommit> {
        let validation = self.pending_target_validations.get(&key.scope)?.clone();
        if validation.sequence != key.sequence {
            return None;
        }
        let scope_is_active = self.scopes.get(&key.scope).is_some_and(|scope| {
            scope.active && scope.lifecycle_generation == validation.lifecycle_generation
        });
        if !scope_is_active || validation.focus_claim_revision != window.focus_claim_revision() {
            self.pending_target_validations.remove(&key.scope);
            return None;
        }
        if validation.rendered_frame_revision == window.rendered_frame_revision()
            || self.latest_pending_sequence().is_some()
        {
            return Some(FocusCommit::RetryAfterFrame);
        }

        let expected = validation.expected_handle.upgrade();
        let target_is_still_current = current.is_some_and(|current| {
            expected.as_ref() == Some(current)
                && self
                    .live_handle_in_scope(&validation.target, &key.scope, window)
                    .as_ref()
                    == Some(current)
        });
        self.pending_target_validations.remove(&key.scope);
        if target_is_still_current {
            return Some(FocusCommit::Preserve);
        }

        Some(
            self.resolve_initial_target(&key.scope, window)
                .map_or(FocusCommit::Blur, FocusCommit::Focus),
        )
    }

    fn latest_pending_sequence(&self) -> Option<u64> {
        self.pending_initial_claim
            .iter()
            .chain(self.pending_restore_claim.iter())
            .map(|claim| claim.sequence)
            .max()
    }

    fn retarget_pending_claims(&mut self, sequence: u64) {
        // One frame callback must remain valid if either arbitration slot is canceled meanwhile.
        if let Some(pending) = self.pending_initial_claim.as_mut() {
            pending.sequence = sequence;
        }
        if let Some(pending) = self.pending_restore_claim.as_mut() {
            pending.sequence = sequence;
        }
    }

    fn resolve_initial_target(
        &self,
        scope_id: &FocusScopeId,
        window: &Window,
    ) -> Option<FocusHandle> {
        let Some(scope) = self.scopes.get(scope_id) else {
            return None;
        };
        if !scope.active {
            return None;
        }
        let Some(root) = scope.root.upgrade() else {
            return None;
        };
        if !window.is_focus_handle_rendered(&root) {
            return None;
        }

        let unavailable_handles = self.unavailable_handles_in_scope(scope_id, window);
        let is_available = |candidate: &FocusHandle| {
            !unavailable_handles
                .iter()
                .any(|unavailable| unavailable == candidate)
        };
        let surface = || {
            scope
                .surface
                .as_ref()
                .and_then(|target| self.live_handle_in_scope(target, scope_id, window))
        };
        let target = match scope.policy.initial_focus() {
            InitialFocusIntent::None => None,
            InitialFocusIntent::FirstFocusable => window
                .first_tab_stop_where_within(&root, is_available)
                .or_else(surface),
            InitialFocusIntent::Target(target) => {
                self.live_handle_in_scope(target, scope_id, window)
            }
            InitialFocusIntent::TargetOrFirstFocusable(target) => self
                .live_handle_in_scope(target, scope_id, window)
                .or_else(|| window.first_tab_stop_where_within(&root, is_available))
                .or_else(surface),
        };
        target
    }

    fn resolve_restore(
        &self,
        scope_id: &FocusScopeId,
        current: Option<&FocusHandle>,
        window: &Window,
    ) -> FocusCommit {
        let Some(scope) = self.scopes.get(scope_id) else {
            return FocusCommit::Preserve;
        };
        if scope.policy.focus_restore() == &FocusRestoreIntent::None {
            return FocusCommit::Preserve;
        }
        let current_is_inside = current.is_some_and(|current| {
            scope
                .root
                .upgrade()
                .is_some_and(|root| root.contains(current, window))
        });
        let trigger = scope
            .trigger
            .as_ref()
            .filter(|target| self.target_is_live(target, window))
            .or_else(|| {
                scope
                    .saved_target
                    .as_ref()
                    .filter(|target| self.target_is_live(target, window))
            });
        let saved_id = match scope.policy.focus_restore() {
            FocusRestoreIntent::None => None,
            FocusRestoreIntent::Trigger => trigger,
            FocusRestoreIntent::Fallback(target) => Some(target),
            FocusRestoreIntent::TriggerOrFallback(fallback) => trigger.or(Some(fallback)),
        };
        let saved = saved_id.map(|target| self.candidate(target, window));

        let mut ancestor_targets = Vec::new();
        let mut parent = scope.policy.parent().cloned();
        while let Some(parent_id) = parent {
            let Some(parent_scope) = self.scopes.get(&parent_id) else {
                break;
            };
            if parent_scope.active {
                if let Some(target) = parent_scope.last_live_target.as_ref() {
                    ancestor_targets.push(self.candidate(target, window));
                }
                if let Some(surface) = parent_scope.surface.as_ref() {
                    ancestor_targets.push(self.candidate(surface, window));
                }
            }
            parent = parent_scope.policy.parent().cloned();
        }

        let window_fallback = self
            .window_fallback
            .as_ref()
            .map(|target| self.candidate(target, window));
        let current_id = (!current_is_inside)
            .then(|| current.and_then(|current| self.target_id_for_handle(current, window)))
            .flatten();
        let current_target = current_id
            .as_ref()
            .map(|target| self.candidate(target, window));
        let has_newer_focus_claim = !current_is_inside
            && scope.activation_focus_claim_revision != window.focus_claim_revision();
        let current_is_live =
            current.is_some_and(|current| window.is_focus_handle_rendered(current));
        let newer_claim = (has_newer_focus_claim && current_is_live)
            .then(|| current_target.as_ref())
            .flatten();
        if has_newer_focus_claim
            && (current.is_none() || current_is_live)
            && !newer_claim.is_some_and(|candidate| candidate.is_available())
        {
            return FocusCommit::Preserve;
        }

        let resolution = resolve_focus_scope_restore(FocusRestoreInput {
            newer_claim,
            saved_target: saved.as_ref(),
            ancestor_targets: &ancestor_targets,
            window_fallback: window_fallback.as_ref(),
            current_target: current_target.as_ref(),
        });
        match resolution {
            FocusResolution::Target(target) => self
                .live_handle(&target, window)
                .map_or(FocusCommit::Blur, FocusCommit::Focus),
            FocusResolution::PreserveCurrent => FocusCommit::Preserve,
            FocusResolution::NoTarget => {
                if current.is_some_and(|current| {
                    !current_is_inside && window.is_focus_handle_rendered(current)
                }) {
                    FocusCommit::Preserve
                } else {
                    FocusCommit::Blur
                }
            }
        }
    }

    fn record_focus(&mut self, current: Option<&FocusHandle>, window: &Window) {
        let Some(current) = current else {
            return;
        };
        let focus_claim_revision = window.focus_claim_revision();
        let focused_scope = self
            .target_id_for_handle(current, window)
            .and_then(|target_id| {
                let scope_id = self.targets.get(&target_id)?.scope.clone()?;
                self.scopes.get_mut(&scope_id)?.last_live_target = Some(target_id);
                Some(scope_id)
            });
        let mut logical_scope = focused_scope;
        while let Some(scope_id) = logical_scope {
            let Some(scope) = self.scopes.get_mut(&scope_id) else {
                break;
            };
            if scope.active {
                scope.activation_focus_claim_revision = focus_claim_revision;
            }
            logical_scope = scope.policy.parent().cloned();
        }

        for scope in self.scopes.values_mut().filter(|scope| scope.active) {
            if scope.root.upgrade().is_some_and(|root| {
                window.is_focus_handle_rendered(&root) && root.contains(current, window)
            }) {
                scope.activation_focus_claim_revision = focus_claim_revision;
            }
        }
    }

    fn target_id_for_handle(&self, handle: &FocusHandle, window: &Window) -> Option<FocusTargetId> {
        self.targets.keys().find_map(|target_id| {
            let live = self.live_handle(target_id, window)?;
            (live == *handle).then(|| target_id.clone())
        })
    }

    fn target_is_live(&self, target: &FocusTargetId, window: &Window) -> bool {
        self.live_handle(target, window).is_some()
    }

    fn live_handle(&self, target: &FocusTargetId, window: &Window) -> Option<FocusHandle> {
        let target_entry = self.targets.get(target)?;
        let handle = target_entry.live_handle(window)?;
        let Some(scope_id) = target_entry.scope.as_ref() else {
            return Some(handle);
        };
        let scope = self.scopes.get(scope_id)?;
        if !scope.active {
            return None;
        }
        let root = scope.root.upgrade()?;
        (window.is_focus_handle_rendered(&root) && root.contains(&handle, window)).then_some(handle)
    }

    fn live_handle_in_scope(
        &self,
        target: &FocusTargetId,
        scope: &FocusScopeId,
        window: &Window,
    ) -> Option<FocusHandle> {
        let target_entry = self.targets.get(target)?;
        let target_scope = target_entry.scope.as_ref()?;
        if !self.scope_contains(scope, target_scope) {
            return None;
        }
        if target_scope != scope
            && !self
                .scopes
                .get(target_scope)
                .is_some_and(|scope| scope.active)
        {
            return None;
        }
        let handle = target_entry.live_handle(window)?;
        let target_root = self.scopes.get(target_scope)?.root.upgrade()?;
        if !window.is_focus_handle_rendered(&target_root) || !target_root.contains(&handle, window)
        {
            return None;
        }
        let root = self.scopes.get(scope)?.root.upgrade()?;
        (window.is_focus_handle_rendered(&root) && root.contains(&handle, window)).then_some(handle)
    }

    fn unavailable_handles_in_scope(
        &self,
        scope: &FocusScopeId,
        window: &Window,
    ) -> Vec<FocusHandle> {
        let mut unavailable = self
            .targets
            .values()
            .filter(|target| {
                target.scope.as_ref().is_some_and(|target_scope| {
                    self.scope_contains(scope, target_scope)
                        && (!target.effective_availability(window).is_available()
                            || (target_scope != scope
                                && !self
                                    .scopes
                                    .get(target_scope)
                                    .is_some_and(|scope| scope.active)))
                })
            })
            .filter_map(|target| target.handle.upgrade())
            .collect::<Vec<_>>();

        for (nested_id, nested) in &self.scopes {
            if nested_id == scope || nested.active || !self.scope_contains(scope, nested_id) {
                continue;
            }
            let Some(root) = nested.root.upgrade() else {
                continue;
            };
            unavailable.extend(window.tab_stops_within(&root));
        }
        let mut unique = Vec::with_capacity(unavailable.len());
        for handle in unavailable {
            if !unique.contains(&handle) {
                unique.push(handle);
            }
        }
        unique
    }

    fn scope_contains(&self, ancestor: &FocusScopeId, scope: &FocusScopeId) -> bool {
        let mut current = Some(scope.clone());
        while let Some(scope_id) = current {
            if &scope_id == ancestor {
                return true;
            }
            current = self
                .scopes
                .get(&scope_id)
                .and_then(|scope| scope.policy.parent().cloned());
        }
        false
    }

    fn candidate(&self, target: &FocusTargetId, window: &Window) -> FocusTargetCandidate {
        let Some(target_entry) = self.targets.get(target) else {
            return FocusTargetCandidate::unavailable(
                target.clone(),
                FocusTargetAvailability::Stale,
            );
        };
        let availability = target_entry.effective_availability(window);
        if !availability.is_available() {
            return FocusTargetCandidate::unavailable(target.clone(), availability);
        }
        if self.live_handle(target, window).is_some() {
            FocusTargetCandidate::available(target.clone())
        } else {
            FocusTargetCandidate::unavailable(target.clone(), FocusTargetAvailability::Stale)
        }
    }
}

#[derive(Clone)]
struct ScopeEntry {
    policy: FocusScopePolicy,
    root: WeakFocusHandle,
    trigger: Option<FocusTargetId>,
    surface: Option<FocusTargetId>,
    active: bool,
    activation_sequence: u64,
    activation_focus_claim_revision: u64,
    lifecycle_generation: u64,
    saved_target: Option<FocusTargetId>,
    last_live_target: Option<FocusTargetId>,
}

#[derive(Clone)]
struct TargetEntry {
    scope: Option<FocusScopeId>,
    handle: WeakFocusHandle,
    availability: FocusTargetAvailability,
}

impl TargetEntry {
    fn effective_availability(&self, window: &Window) -> FocusTargetAvailability {
        if !self.availability.is_available() {
            return self.availability;
        }
        let Some(handle) = self.handle.upgrade() else {
            return FocusTargetAvailability::Stale;
        };
        if window.is_focus_handle_rendered(&handle) {
            FocusTargetAvailability::Available
        } else {
            FocusTargetAvailability::Unmounted
        }
    }

    fn live_handle(&self, window: &Window) -> Option<FocusHandle> {
        self.effective_availability(window)
            .is_available()
            .then(|| self.handle.upgrade())
            .flatten()
    }
}

#[derive(Clone)]
struct PendingFocusClaim {
    sequence: u64,
    focus_claim_revision: u64,
    rendered_frame_revision: u64,
    kind: PendingFocusClaimKind,
}

#[derive(Clone)]
struct PendingTargetValidation {
    sequence: u64,
    lifecycle_generation: u64,
    target: FocusTargetId,
    expected_handle: WeakFocusHandle,
    focus_claim_revision: u64,
    rendered_frame_revision: u64,
}

#[derive(Clone)]
struct PendingTargetValidationKey {
    scope: FocusScopeId,
    sequence: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct ScheduledFocusClaimRetry {
    scope: FocusScopeId,
    sequence: u64,
    frame_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingClaimRetry {
    pending: bool,
    sequence: Option<u64>,
}

impl PendingClaimRetry {
    const fn not_pending() -> Self {
        Self {
            pending: false,
            sequence: None,
        }
    }

    const fn already_scheduled() -> Self {
        Self {
            pending: true,
            sequence: None,
        }
    }

    const fn scheduled(sequence: u64) -> Self {
        Self {
            pending: true,
            sequence: Some(sequence),
        }
    }

    pub(crate) const fn is_pending(self) -> bool {
        self.pending
    }

    pub(crate) const fn was_scheduled(self) -> bool {
        self.sequence.is_some()
    }
}

#[derive(Clone)]
enum PendingFocusClaimKind {
    Initial {
        scope: FocusScopeId,
        lifecycle_generation: u64,
    },
    Restore {
        scope: FocusScopeId,
        lifecycle_generation: u64,
    },
}

impl PendingFocusClaimKind {
    fn is_initial(&self) -> bool {
        matches!(self, Self::Initial { .. })
    }

    fn scope(&self) -> &FocusScopeId {
        match self {
            Self::Initial { scope, .. } | Self::Restore { scope, .. } => scope,
        }
    }
}

enum FocusCommit {
    Focus(FocusHandle),
    Preserve,
    Blur,
    RetryAfterFrame,
}

enum ModalTraversal {
    Pending,
    Live {
        root: FocusHandle,
        unavailable_handles: Vec<FocusHandle>,
        surface: Option<FocusHandle>,
    },
}

#[cfg(test)]
#[path = "focus_scope_tests.rs"]
mod tests;
