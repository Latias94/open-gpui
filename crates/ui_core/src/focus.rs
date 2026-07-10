//! Renderer-neutral focus vocabulary used by the Open GPUI component ecosystem.

use std::fmt;

/// Stable semantic identity for a focus target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FocusTargetId {
    id: String,
}

impl FocusTargetId {
    /// Creates a focus target id from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Returns the stable target id.
    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl From<&str> for FocusTargetId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FocusTargetId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for FocusTargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id)
    }
}

/// Stable semantic identity for a focus scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FocusScopeId {
    id: String,
}

impl FocusScopeId {
    /// Creates a focus scope id from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Returns the stable scope id.
    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl From<&str> for FocusScopeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FocusScopeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for FocusScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id)
    }
}

/// Focus behavior owned by a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusScopeMode {
    /// The scope records targets without changing ordinary window traversal.
    Passive,
    /// The scope contains forward and reverse traversal while it is the innermost active modal.
    ModalLoop,
}

/// Initial focus behavior when a scope becomes active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialFocusIntent {
    /// Preserve the current focus.
    None,
    /// Focus the first available descendant, then the live scope surface.
    FirstFocusable,
    /// Focus a specific target without an implicit fallback.
    Target(FocusTargetId),
    /// Prefer a specific target, then the first available descendant, then the live surface.
    TargetOrFirstFocusable(FocusTargetId),
}

impl InitialFocusIntent {
    /// Returns a stable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::FirstFocusable => "first focusable",
            Self::Target(_) => "target",
            Self::TargetOrFirstFocusable(_) => "target or first focusable",
        }
    }
}

/// Focus restoration behavior after a scope closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRestoreIntent {
    /// Do not restore focus.
    None,
    /// Restore the focus target saved when the scope opened.
    Trigger,
    /// Restore a named fallback target.
    Fallback(FocusTargetId),
    /// Prefer the saved target and fall back to a named target.
    TriggerOrFallback(FocusTargetId),
}

impl FocusRestoreIntent {
    /// Returns a stable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trigger => "trigger",
            Self::Fallback(_) => "fallback",
            Self::TriggerOrFallback(_) => "trigger or fallback",
        }
    }
}

/// Renderer-neutral policy for one focus scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusScopePolicy {
    id: FocusScopeId,
    parent: Option<FocusScopeId>,
    mode: FocusScopeMode,
    initial_focus: InitialFocusIntent,
    focus_restore: FocusRestoreIntent,
}

impl FocusScopePolicy {
    /// Creates a focus scope policy.
    pub fn new(id: impl Into<FocusScopeId>, mode: FocusScopeMode) -> Self {
        Self {
            id: id.into(),
            parent: None,
            mode,
            initial_focus: InitialFocusIntent::None,
            focus_restore: FocusRestoreIntent::None,
        }
    }

    /// Sets the logical parent scope.
    pub fn with_parent(mut self, parent: impl Into<FocusScopeId>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Sets the initial focus policy.
    pub fn with_initial_focus(mut self, initial_focus: InitialFocusIntent) -> Self {
        self.initial_focus = initial_focus;
        self
    }

    /// Sets the focus restoration policy.
    pub fn with_focus_restore(mut self, focus_restore: FocusRestoreIntent) -> Self {
        self.focus_restore = focus_restore;
        self
    }

    /// Returns the scope identity.
    pub const fn id(&self) -> &FocusScopeId {
        &self.id
    }

    /// Returns the logical parent scope.
    pub const fn parent(&self) -> Option<&FocusScopeId> {
        self.parent.as_ref()
    }

    /// Returns the scope behavior.
    pub const fn mode(&self) -> FocusScopeMode {
        self.mode
    }

    /// Returns the initial focus policy.
    pub const fn initial_focus(&self) -> &InitialFocusIntent {
        &self.initial_focus
    }

    /// Returns the focus restoration policy.
    pub const fn focus_restore(&self) -> &FocusRestoreIntent {
        &self.focus_restore
    }
}

/// Availability of a target in the current renderer snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTargetAvailability {
    /// The target may receive focus.
    Available,
    /// The target is disabled.
    Disabled,
    /// The target is hidden.
    Hidden,
    /// The target is not mounted in the current frame.
    Unmounted,
    /// The registration was superseded by a newer frame or logical instance.
    Stale,
}

impl FocusTargetAvailability {
    /// Returns whether this target may receive focus.
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// One ordered logical focus candidate projected by a renderer adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTargetCandidate {
    id: FocusTargetId,
    availability: FocusTargetAvailability,
}

impl FocusTargetCandidate {
    /// Creates an available focus candidate.
    pub fn available(id: impl Into<FocusTargetId>) -> Self {
        Self {
            id: id.into(),
            availability: FocusTargetAvailability::Available,
        }
    }

    /// Creates an unavailable focus candidate with a diagnostic reason.
    pub fn unavailable(
        id: impl Into<FocusTargetId>,
        availability: FocusTargetAvailability,
    ) -> Self {
        debug_assert_ne!(availability, FocusTargetAvailability::Available);
        Self {
            id: id.into(),
            availability,
        }
    }

    /// Returns the stable target identity.
    pub const fn id(&self) -> &FocusTargetId {
        &self.id
    }

    /// Returns the current renderer-projected availability.
    pub const fn availability(&self) -> FocusTargetAvailability {
        self.availability
    }

    /// Returns whether the candidate may receive focus.
    pub const fn is_available(&self) -> bool {
        self.availability.is_available()
    }
}

/// Renderer-neutral result of resolving a focus request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusResolution {
    /// Move focus to the resolved logical target.
    Target(FocusTargetId),
    /// Keep a still-live current focus without synthesizing a new target.
    PreserveCurrent,
    /// No safe focus target exists.
    NoTarget,
}

/// Inputs used to resolve a close-commit restoration claim.
#[derive(Debug, Clone, Copy)]
pub struct FocusRestoreInput<'a> {
    /// A newer focus claim that supersedes this restoration when live.
    pub newer_claim: Option<&'a FocusTargetCandidate>,
    /// The target saved when the closing scope opened.
    pub saved_target: Option<&'a FocusTargetCandidate>,
    /// Last-live targets from active ancestors, nearest ancestor first.
    pub ancestor_last_targets: &'a [FocusTargetCandidate],
    /// Explicitly registered application fallback for this window.
    pub window_fallback: Option<&'a FocusTargetCandidate>,
    /// Focus observed when the restoration is committed.
    pub current_target: Option<&'a FocusTargetCandidate>,
}

/// Resolves a close-commit restoration claim using deterministic priority.
pub fn resolve_focus_restore(input: FocusRestoreInput<'_>) -> FocusResolution {
    available(input.newer_claim)
        .or_else(|| available(input.saved_target))
        .or_else(|| first_available(input.ancestor_last_targets))
        .or_else(|| available(input.window_fallback))
        .map(target_resolution)
        .unwrap_or_else(|| {
            if available(input.current_target).is_some() {
                FocusResolution::PreserveCurrent
            } else {
                FocusResolution::NoTarget
            }
        })
}

fn first_available(candidates: &[FocusTargetCandidate]) -> Option<&FocusTargetCandidate> {
    candidates.iter().find(|candidate| candidate.is_available())
}

fn available(candidate: Option<&FocusTargetCandidate>) -> Option<&FocusTargetCandidate> {
    candidate.filter(|candidate| candidate.is_available())
}

fn target_resolution(candidate: &FocusTargetCandidate) -> FocusResolution {
    FocusResolution::Target(candidate.id().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(id: &str) -> FocusTargetCandidate {
        FocusTargetCandidate::available(id)
    }

    fn unavailable(id: &str, availability: FocusTargetAvailability) -> FocusTargetCandidate {
        FocusTargetCandidate::unavailable(id, availability)
    }

    #[test]
    fn focus_scope_policy_records_nesting_and_modal_mode() {
        let policy = FocusScopePolicy::new("child", FocusScopeMode::ModalLoop)
            .with_parent("parent")
            .with_initial_focus(InitialFocusIntent::TargetOrFirstFocusable(
                FocusTargetId::new("preferred"),
            ))
            .with_focus_restore(FocusRestoreIntent::TriggerOrFallback(FocusTargetId::new(
                "window-fallback",
            )));

        assert_eq!(policy.id().as_str(), "child");
        assert_eq!(policy.parent().map(FocusScopeId::as_str), Some("parent"));
        assert_eq!(policy.mode(), FocusScopeMode::ModalLoop);
        assert_eq!(
            policy.initial_focus(),
            &InitialFocusIntent::TargetOrFirstFocusable(FocusTargetId::new("preferred"))
        );
        assert_eq!(
            policy.focus_restore(),
            &FocusRestoreIntent::TriggerOrFallback(FocusTargetId::new("window-fallback"))
        );
    }

    #[test]
    fn restore_prefers_new_claim_saved_ancestor_and_window_fallback() {
        let newer = available("newer");
        let saved = available("saved");
        let stale_ancestor = unavailable("stale-parent", FocusTargetAvailability::Stale);
        let live_ancestor = available("live-parent");
        let fallback = available("window-fallback");
        let current = available("current");
        let ancestors = [stale_ancestor, live_ancestor];

        let resolve = |newer_claim, saved_target, ancestor_last_targets, window_fallback| {
            resolve_focus_restore(FocusRestoreInput {
                newer_claim,
                saved_target,
                ancestor_last_targets,
                window_fallback,
                current_target: Some(&current),
            })
        };

        assert_eq!(
            resolve(Some(&newer), Some(&saved), &ancestors, Some(&fallback)),
            FocusResolution::Target(FocusTargetId::new("newer"))
        );
        assert_eq!(
            resolve(None, Some(&saved), &ancestors, Some(&fallback)),
            FocusResolution::Target(FocusTargetId::new("saved"))
        );
        assert_eq!(
            resolve(None, None, &ancestors, Some(&fallback)),
            FocusResolution::Target(FocusTargetId::new("live-parent"))
        );
        assert_eq!(
            resolve(None, None, &ancestors[..1], Some(&fallback)),
            FocusResolution::Target(FocusTargetId::new("window-fallback"))
        );
        assert_eq!(
            resolve(None, None, &ancestors[..1], None),
            FocusResolution::PreserveCurrent
        );
    }
}
