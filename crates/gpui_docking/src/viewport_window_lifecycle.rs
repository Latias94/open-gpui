use crate::{
    DockSpaceId, DockViewportActivationTransaction, DockViewportCloseOutcome,
    DockViewportClosePlanState, DockViewportCloseStatus, DockViewportFocusRequest,
    DockViewportMergeBackClosePlan, DockViewportShouldCloseOutcome, DockViewportWindowCloseEffect,
    DockViewportWindowEffects, viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::AnyWindowHandle;

pub(crate) struct DockViewportWindowLifecycleController;

#[derive(Default)]
pub(crate) struct DockViewportReplacementCleanup {
    pub(crate) replaced_windows: Vec<DockViewportWindowCloseEffect>,
    pub(crate) affected_windows: Vec<AnyWindowHandle>,
}

pub(crate) struct DockViewportUnregisteredSpace {
    pub(crate) window: AnyWindowHandle,
    pub(crate) affected_windows: Vec<AnyWindowHandle>,
}

#[derive(Default)]
pub(crate) struct DockViewportVacatedTearOffSource {
    pub(crate) changed: bool,
    pub(crate) windows: Vec<DockViewportWindowCloseEffect>,
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
}

impl DockViewportWindowLifecycleController {
    pub(crate) fn complete_pending_close_plan(
        close: DockViewportClosedWindowRefresh,
        pending_state: Option<DockViewportClosePlanState>,
        commit_pending_plan: impl FnOnce(&DockViewportMergeBackClosePlan) -> DockViewportCloseStatus,
    ) -> DockViewportClosedWindowRefresh {
        let outcome = match pending_state {
            Some(DockViewportClosePlanState::Pending(plan)) if close.outcome.space().is_some() => {
                let close_status = commit_pending_plan(&plan);
                if close_status == DockViewportCloseStatus::MergedBack {
                    close.outcome.with_merge_back(plan)
                } else {
                    close.outcome.with_status(close_status)
                }
            }
            Some(DockViewportClosePlanState::Discarded) => close
                .outcome
                .with_status(DockViewportCloseStatus::MergeBackFailed),
            _ => close.outcome,
        };
        DockViewportClosedWindowRefresh::new(outcome, close.window_effects)
    }

    pub(crate) fn close_recovery_activation(
        outcome: &DockViewportCloseOutcome,
        reusable: DockViewportReusableWindowOutcome,
    ) -> DockViewportCloseRecoveryActivation {
        let Some((target_space, focus_request)) = Self::close_recovery_request_parts(outcome)
        else {
            return DockViewportCloseRecoveryActivation::none();
        };
        let (window, window_effects) = reusable.into_parts();
        let activation = match window {
            DockViewportReusableWindow::Reused {
                registration,
                window,
            } if registration.space() == &target_space => {
                Some(DockViewportActivationTransaction::close_recovery(
                    registration,
                    window,
                    focus_request,
                ))
            }
            DockViewportReusableWindow::Missing
            | DockViewportReusableWindow::Stale
            | DockViewportReusableWindow::Reused { .. } => None,
        };
        DockViewportCloseRecoveryActivation::new(activation, window_effects)
    }

    pub(crate) fn drop_activation(
        reusable: DockViewportReusableWindowOutcome,
        focus_request: DockViewportFocusRequest,
    ) -> (
        Option<DockViewportActivationTransaction>,
        DockViewportWindowEffects,
    ) {
        let (window, window_effects) = reusable.into_parts();
        let activation = match window {
            DockViewportReusableWindow::Reused {
                registration,
                window,
            } => Some(DockViewportActivationTransaction::registered(
                registration,
                window,
                focus_request,
            )),
            DockViewportReusableWindow::Missing | DockViewportReusableWindow::Stale => None,
        };
        (activation, window_effects)
    }

    fn close_recovery_request_parts(
        outcome: &DockViewportCloseOutcome,
    ) -> Option<(DockSpaceId, DockViewportFocusRequest)> {
        if outcome.status() != DockViewportCloseStatus::MergedBack {
            return None;
        }
        let target_space = outcome.merge_target_space().cloned()?;
        let focus_request = outcome.focus_item().cloned().map_or_else(
            DockViewportFocusRequest::no_panel_focus,
            DockViewportFocusRequest::panel,
        );
        Some((target_space, focus_request))
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

pub(crate) enum DockViewportReusableWindow {
    Missing,
    Reused {
        registration: DockViewportRegistrationKey,
        window: AnyWindowHandle,
    },
    Stale,
}

pub(crate) struct DockViewportReusableWindowOutcome {
    window: DockViewportReusableWindow,
    window_effects: DockViewportWindowEffects,
    topology_changed: bool,
}

impl DockViewportReusableWindowOutcome {
    pub(crate) fn missing() -> Self {
        Self {
            window: DockViewportReusableWindow::Missing,
            window_effects: DockViewportWindowEffects::default(),
            topology_changed: false,
        }
    }

    pub(crate) fn reused(
        registration: DockViewportRegistrationKey,
        window: AnyWindowHandle,
    ) -> Self {
        debug_assert_eq!(
            registration.window_id(),
            window.window_id(),
            "reused viewport lease must belong to its window"
        );
        Self {
            window: DockViewportReusableWindow::Reused {
                registration,
                window,
            },
            window_effects: DockViewportWindowEffects::default(),
            topology_changed: false,
        }
    }

    pub(crate) fn stale() -> Self {
        Self {
            window: DockViewportReusableWindow::Stale,
            window_effects: DockViewportWindowEffects::default(),
            topology_changed: false,
        }
    }

    pub(crate) fn stale_with_affected_windows(affected_windows: Vec<AnyWindowHandle>) -> Self {
        Self {
            window: DockViewportReusableWindow::Stale,
            window_effects: DockViewportWindowEffects::new(
                Vec::new(),
                affected_windows,
                Vec::new(),
            ),
            topology_changed: true,
        }
    }

    pub(crate) fn topology_changed(&self) -> bool {
        self.topology_changed
    }

    pub(crate) fn into_parts(self) -> (DockViewportReusableWindow, DockViewportWindowEffects) {
        (self.window, self.window_effects)
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
        viewport_registry::DockViewportRegistrationKey,
        viewport_test_support::{handle, space},
    };

    #[test]
    fn reusable_window_outcome_builds_drop_activation() {
        let target_space = space("target");
        let target_window = handle(7);
        let focus_item = DockItemId::from("panel-a");
        let focus_request = DockViewportFocusRequest::panel(focus_item.clone());
        let registration =
            DockViewportRegistrationKey::for_test(target_space.clone(), target_window.window_id());

        let (activation, window_effects) = DockViewportWindowLifecycleController::drop_activation(
            DockViewportReusableWindowOutcome::reused(registration.clone(), target_window),
            focus_request,
        );

        assert_eq!(window_effects, DockViewportWindowEffects::default());
        let activation = activation.expect("reused window should create drop activation");
        assert_eq!(activation.space(), &target_space);
        assert_eq!(activation.window(), target_window);
        assert_eq!(activation.registration_key(), &registration);
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
        let registration =
            DockViewportRegistrationKey::for_test(target_space.clone(), target_window.window_id());
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

        let recovery = DockViewportWindowLifecycleController::close_recovery_activation(
            &close,
            DockViewportReusableWindowOutcome::reused(registration.clone(), target_window),
        );

        assert_eq!(
            recovery.window_effects(),
            DockViewportWindowEffects::default()
        );
        let activation = recovery
            .activation
            .expect("reused target window should create recovery activation");
        assert_eq!(activation.space(), &target_space);
        assert_eq!(activation.window(), target_window);
        assert_eq!(activation.registration_key(), &registration);
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
        let (activation, window_effects) = DockViewportWindowLifecycleController::drop_activation(
            DockViewportReusableWindowOutcome::stale_with_affected_windows(vec![refresh]),
            DockViewportFocusRequest::no_panel_focus(),
        );

        assert!(activation.is_none());
        assert_eq!(window_effects.refresh(), &[refresh]);
    }
}
