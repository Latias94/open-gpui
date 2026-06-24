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
