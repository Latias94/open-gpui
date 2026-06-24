use crate::{DockViewportCloseOutcome, DockViewportShouldCloseOutcome, DockViewportWindowEffects};
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
