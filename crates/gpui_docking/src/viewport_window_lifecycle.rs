use crate::{
    DockSpaceId, DockViewportActivationTransaction, DockViewportCloseOutcome,
    DockViewportClosePlanState, DockViewportCloseStatus, DockViewportFocusRequest,
    DockViewportMergeBackClosePlan, DockViewportShouldCloseOutcome, DockViewportWindowEffects,
};
use open_gpui::AnyWindowHandle;

#[derive(Default)]
pub(crate) struct DockViewportReplacementCleanup {
    pub(crate) replaced_windows: Vec<AnyWindowHandle>,
    pub(crate) affected_windows: Vec<AnyWindowHandle>,
}

pub(crate) struct DockViewportUnregisteredSpace {
    pub(crate) window: AnyWindowHandle,
    pub(crate) affected_windows: Vec<AnyWindowHandle>,
}

#[derive(Default)]
pub(crate) struct DockViewportVacatedTearOffSource {
    pub(crate) windows: Vec<AnyWindowHandle>,
    pub(crate) affected_windows: Vec<AnyWindowHandle>,
}

pub(crate) struct DockViewportClosedWindowRefresh {
    pub(crate) outcome: DockViewportCloseOutcome,
    window_effects: DockViewportWindowEffects,
}

pub(crate) struct DockViewportShouldCloseRefresh {
    pub(crate) outcome: DockViewportShouldCloseOutcome,
    window_effects: DockViewportWindowEffects,
}

pub(crate) struct DockViewportCloseRecoveryActivation {
    pub(crate) activation: Option<crate::DockViewportActivationTransaction>,
    window_effects: DockViewportWindowEffects,
}

impl DockViewportClosedWindowRefresh {
    pub(crate) fn new(
        outcome: DockViewportCloseOutcome,
        window_effects: DockViewportWindowEffects,
    ) -> Self {
        Self {
            outcome,
            window_effects,
        }
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }

    pub(crate) fn complete_pending_close_plan(
        self,
        pending_state: Option<DockViewportClosePlanState>,
        commit_pending_plan: impl FnOnce(&DockViewportMergeBackClosePlan) -> DockViewportCloseStatus,
    ) -> Self {
        let outcome = match pending_state {
            Some(DockViewportClosePlanState::Pending(plan)) if self.outcome.space().is_some() => {
                let close_status = commit_pending_plan(&plan);
                if close_status == DockViewportCloseStatus::MergedBack {
                    self.outcome.with_merge_back(plan)
                } else {
                    self.outcome.with_status(close_status)
                }
            }
            Some(DockViewportClosePlanState::Discarded) => self
                .outcome
                .with_status(DockViewportCloseStatus::MergeBackFailed),
            _ => self.outcome,
        };
        Self::new(outcome, self.window_effects)
    }
}

impl DockViewportShouldCloseRefresh {
    pub(crate) fn new(
        outcome: DockViewportShouldCloseOutcome,
        window_effects: DockViewportWindowEffects,
    ) -> Self {
        Self {
            outcome,
            window_effects,
        }
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }
}

impl DockViewportCloseRecoveryActivation {
    pub(crate) fn new(
        activation: Option<crate::DockViewportActivationTransaction>,
        window_effects: DockViewportWindowEffects,
    ) -> Self {
        Self {
            activation,
            window_effects,
        }
    }

    pub(crate) fn none() -> Self {
        Self {
            activation: None,
            window_effects: DockViewportWindowEffects::default(),
        }
    }

    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }
}

pub(crate) struct DockViewportCloseRecoveryRequest {
    target_space: DockSpaceId,
    focus_request: DockViewportFocusRequest,
}

impl DockViewportCloseRecoveryRequest {
    pub(crate) fn from_close_outcome(outcome: &DockViewportCloseOutcome) -> Option<Self> {
        if outcome.status() != DockViewportCloseStatus::MergedBack {
            return None;
        }
        let target_space = outcome.merge_target_space().cloned()?;
        let focus_request = outcome.focus_item().cloned().map_or_else(
            DockViewportFocusRequest::no_panel_focus,
            DockViewportFocusRequest::panel,
        );
        Some(Self {
            target_space,
            focus_request,
        })
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn into_activation(
        self,
        reusable: DockViewportReusableWindow,
        window_effects: DockViewportWindowEffects,
    ) -> DockViewportCloseRecoveryActivation {
        DockViewportCloseRecoveryActivation::new(
            reusable.into_close_recovery_activation(self.target_space, self.focus_request),
            window_effects,
        )
    }
}

pub(crate) enum DockViewportReusableWindow {
    Missing,
    Reused(AnyWindowHandle),
    Stale,
}

impl DockViewportReusableWindow {
    fn into_close_recovery_activation(
        self,
        space: DockSpaceId,
        focus_request: DockViewportFocusRequest,
    ) -> Option<DockViewportActivationTransaction> {
        match self {
            Self::Reused(window) => Some(DockViewportActivationTransaction::close_recovery(
                space,
                window,
                focus_request,
            )),
            Self::Missing | Self::Stale => None,
        }
    }

    fn into_drop_activation(
        self,
        space: DockSpaceId,
        focus_request: DockViewportFocusRequest,
    ) -> Option<DockViewportActivationTransaction> {
        match self {
            Self::Reused(window) => Some(DockViewportActivationTransaction::new(
                space,
                window,
                focus_request,
            )),
            Self::Missing | Self::Stale => None,
        }
    }
}

pub(crate) struct DockViewportReusableWindowOutcome {
    window: DockViewportReusableWindow,
    window_effects: DockViewportWindowEffects,
}

impl DockViewportReusableWindowOutcome {
    pub(crate) fn missing() -> Self {
        Self {
            window: DockViewportReusableWindow::Missing,
            window_effects: DockViewportWindowEffects::default(),
        }
    }

    pub(crate) fn reused(window: AnyWindowHandle) -> Self {
        Self {
            window: DockViewportReusableWindow::Reused(window),
            window_effects: DockViewportWindowEffects::default(),
        }
    }

    pub(crate) fn stale() -> Self {
        Self::stale_with_affected_windows(Vec::new())
    }

    pub(crate) fn stale_with_affected_windows(affected_windows: Vec<AnyWindowHandle>) -> Self {
        Self {
            window: DockViewportReusableWindow::Stale,
            window_effects: DockViewportWindowEffects::new(
                Vec::new(),
                affected_windows,
                Vec::new(),
            ),
        }
    }

    pub(crate) fn into_parts(self) -> (DockViewportReusableWindow, DockViewportWindowEffects) {
        (self.window, self.window_effects)
    }

    pub(crate) fn into_drop_activation(
        self,
        space: DockSpaceId,
        focus_request: DockViewportFocusRequest,
    ) -> (
        Option<DockViewportActivationTransaction>,
        DockViewportWindowEffects,
    ) {
        let (window, window_effects) = self.into_parts();
        (
            window.into_drop_activation(space, focus_request),
            window_effects,
        )
    }

    pub(crate) fn into_close_recovery_activation(
        self,
        request: DockViewportCloseRecoveryRequest,
    ) -> DockViewportCloseRecoveryActivation {
        let (reusable, window_effects) = self.into_parts();
        request.into_activation(reusable, window_effects)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportSpaceFocusCleanup {
    Remove,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportRuntimeWindowStateCleanup {
    SpaceUnregistered,
    ReplacedSameSpaceMapping,
    ReplacedDifferentSpaceMapping,
    ClosedWindow,
}

impl DockViewportRuntimeWindowStateCleanup {
    pub(crate) fn discard_close_plan(self) -> bool {
        !matches!(self, Self::ClosedWindow)
    }

    pub(crate) fn focus_cleanup(self) -> DockViewportSpaceFocusCleanup {
        match self {
            Self::ReplacedSameSpaceMapping => DockViewportSpaceFocusCleanup::Preserve,
            Self::SpaceUnregistered | Self::ReplacedDifferentSpaceMapping | Self::ClosedWindow => {
                DockViewportSpaceFocusCleanup::Remove
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockItemId, DockViewportCloseOutcome, DockViewportCloseStatus,
        DockViewportMergeBackClosePlan, DockViewportWindowActivation,
        viewport_test_support::{handle, space},
    };

    #[test]
    fn reusable_window_outcome_builds_drop_activation() {
        let target_space = space("target");
        let target_window = handle(7);
        let focus_item = DockItemId::from("panel-a");
        let focus_request = DockViewportFocusRequest::panel(focus_item.clone());

        let (activation, window_effects) = DockViewportReusableWindowOutcome::reused(target_window)
            .into_drop_activation(target_space.clone(), focus_request);

        assert_eq!(window_effects, DockViewportWindowEffects::default());
        let activation = activation.expect("reused window should create drop activation");
        assert_eq!(activation.space(), &target_space);
        assert_eq!(activation.window(), target_window);
        assert_eq!(
            activation.window_activation(),
            DockViewportWindowActivation::Request
        );
        assert_eq!(
            activation.focus_request(),
            &DockViewportFocusRequest::panel(focus_item)
        );
    }

    #[test]
    fn reusable_window_outcome_builds_close_recovery_activation() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(11);
        let focus_item = DockItemId::from("panel-b");
        let close = DockViewportCloseOutcome::new(
            Some(source_space.clone()),
            handle(5).window_id(),
            DockViewportCloseStatus::Closed,
        )
        .with_merge_back(DockViewportMergeBackClosePlan::new(
            source_space,
            target_space.clone(),
            Some(focus_item.clone()),
        ));
        let request = DockViewportCloseRecoveryRequest::from_close_outcome(&close)
            .expect("merged close should request recovery");

        let recovery = DockViewportReusableWindowOutcome::reused(target_window)
            .into_close_recovery_activation(request);

        assert_eq!(
            recovery.window_effects(),
            DockViewportWindowEffects::default()
        );
        let activation = recovery
            .activation
            .expect("reused target window should create recovery activation");
        assert_eq!(activation.space(), &target_space);
        assert_eq!(activation.window(), target_window);
        assert_eq!(
            activation.window_activation(),
            DockViewportWindowActivation::DoNotRequest
        );
        assert_eq!(
            activation.focus_request(),
            &DockViewportFocusRequest::panel(focus_item)
        );
    }

    #[test]
    fn stale_reusable_window_outcome_keeps_cleanup_effects_without_activation() {
        let refresh = handle(13);
        let (activation, window_effects) =
            DockViewportReusableWindowOutcome::stale_with_affected_windows(vec![refresh])
                .into_drop_activation(space("target"), DockViewportFocusRequest::no_panel_focus());

        assert!(activation.is_none());
        assert_eq!(window_effects.refresh(), &[refresh]);
    }
}
