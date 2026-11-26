//! Root-level motion facade for duration-first adapter-owned animation.

use crate::motion::{
    MotionDuration, MotionEasing, MotionModel, MotionPolicyContext, MotionPolicyInput,
    MotionPolicyReport, MotionPreference, MotionScalarSample, MotionSpec, MotionSpringOptions,
    MotionSpringSpec, validate_motion_policy,
};
use std::time::Duration;

/// Product-level reason for running motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionIntent {
    /// Pointer-coupled drag or resize feedback.
    PointerInput,
    /// Keyboard focus movement or command focus feedback.
    KeyboardFocus,
    /// Preview chrome tied to a semantic target.
    VisualAffordance,
    /// Lightweight hover, guide, or presence feedback.
    Affordance,
    /// Committed layout change such as insert, remove, collapse, or expand.
    CommittedLayout,
    /// Continuity motion for retargeted pane, divider, viewport, or zoom transitions.
    Continuity,
    /// Decorative motion that does not communicate layout or target semantics.
    Decorative,
}

impl MotionIntent {
    /// Returns the policy context associated with this intent.
    pub const fn policy_context(self) -> MotionPolicyContext {
        match self {
            Self::PointerInput => MotionPolicyContext::PointerDrag,
            Self::KeyboardFocus => MotionPolicyContext::KeyboardFocus,
            Self::VisualAffordance => MotionPolicyContext::VisualAffordancePreview,
            Self::Affordance => MotionPolicyContext::AffordancePresence,
            Self::CommittedLayout => MotionPolicyContext::CommittedLayout,
            Self::Continuity => MotionPolicyContext::Continuity,
            Self::Decorative => MotionPolicyContext::Decorative,
        }
    }

    const fn default_spatial_motion(self) -> bool {
        matches!(
            self,
            Self::VisualAffordance | Self::CommittedLayout | Self::Continuity
        )
    }
}

/// Duration-first transition selected by product intent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionTransition {
    intent: MotionIntent,
    model: MotionModel,
    spatial_motion: bool,
    reduced_motion_final_state: bool,
}

impl MotionTransition {
    /// Creates an immediate transition.
    pub fn immediate() -> Self {
        Self::from_model(
            MotionIntent::Affordance,
            MotionModel::timeline(MotionSpec::immediate()),
        )
        .with_spatial_motion(false)
    }

    /// Creates a duration/easing transition for an intent.
    pub fn duration(
        intent: MotionIntent,
        preference: MotionPreference,
        duration: MotionDuration,
        easing: MotionEasing,
    ) -> Self {
        Self::from_model(
            intent,
            MotionModel::timeline(MotionSpec::new(preference, duration, easing)),
        )
    }

    /// Creates a spring transition for an intent.
    ///
    /// The configuration carried by `options` retains [`crate::motion::SpringConfig`]'s preconditions.
    /// Its epsilon controls settling; its review hints participate only in domain policy.
    pub fn spring(
        intent: MotionIntent,
        preference: MotionPreference,
        options: MotionSpringOptions,
    ) -> Self {
        Self::from_model(
            intent,
            MotionModel::spring(MotionSpringSpec::from_options(preference, options)),
        )
    }

    /// Creates the standard short affordance transition.
    pub fn affordance(preference: MotionPreference) -> Self {
        Self::from_model(
            MotionIntent::Affordance,
            MotionModel::spring(MotionSpringSpec::affordance(preference)),
        )
    }

    /// Creates the standard visual-affordance preview transition.
    pub fn visual_affordance(preference: MotionPreference) -> Self {
        Self::from_model(
            MotionIntent::VisualAffordance,
            MotionModel::spring(MotionSpringSpec::affordance(preference)),
        )
    }

    /// Creates the standard committed-layout transition.
    pub fn committed_layout(preference: MotionPreference) -> Self {
        Self::from_model(
            MotionIntent::CommittedLayout,
            MotionModel::spring(MotionSpringSpec::layout(preference)),
        )
    }

    /// Creates the standard continuity transition.
    pub fn continuity(preference: MotionPreference) -> Self {
        Self::from_model(
            MotionIntent::Continuity,
            MotionModel::spring(MotionSpringSpec::continuity(preference)),
        )
    }

    pub(crate) fn from_model(intent: MotionIntent, model: MotionModel) -> Self {
        Self {
            intent,
            model,
            spatial_motion: intent.default_spatial_motion(),
            reduced_motion_final_state: true,
        }
    }

    /// Returns the product intent.
    pub const fn intent(self) -> MotionIntent {
        self.intent
    }

    /// Returns the motion preference carried by this transition.
    pub const fn preference(self) -> MotionPreference {
        self.model.preference()
    }

    /// Returns whether this transition participates in spatial motion policy.
    pub const fn spatial_motion(self) -> bool {
        self.spatial_motion
    }

    /// Returns whether reduced motion preserves the final semantic state.
    pub const fn reduced_motion_final_state(self) -> bool {
        self.reduced_motion_final_state
    }

    /// Returns a copy with explicit spatial-motion policy participation.
    pub const fn with_spatial_motion(mut self, spatial_motion: bool) -> Self {
        self.spatial_motion = spatial_motion;
        self
    }

    /// Returns a copy with explicit reduced-motion final-state coverage.
    pub const fn with_reduced_motion_final_state(
        mut self,
        reduced_motion_final_state: bool,
    ) -> Self {
        self.reduced_motion_final_state = reduced_motion_final_state;
        self
    }

    /// Returns whether this transition completes immediately before policy resolution.
    pub const fn is_immediate(self) -> bool {
        self.model.is_immediate()
    }

    pub(crate) const fn policy_input(self) -> MotionPolicyInput {
        MotionPolicyInput::new(self.intent.policy_context(), self.model)
            .with_spatial_motion(self.spatial_motion)
            .with_reduced_motion_final_state(self.reduced_motion_final_state)
    }

    fn resolved_model(self) -> MotionModel {
        if self.policy_report().is_ok() {
            self.model
        } else {
            MotionModel::timeline(MotionSpec::immediate())
        }
    }

    /// Returns the policy report for this transition.
    pub fn policy_report(self) -> MotionPolicyReport {
        validate_motion_policy(self.policy_input())
    }

    /// Samples this transition as a scalar value using explicit elapsed time.
    pub fn sample_scalar_elapsed(
        self,
        from: f32,
        target: f32,
        initial_velocity: f32,
        elapsed: Duration,
    ) -> MotionScalarSample {
        self.resolved_model()
            .sample_scalar_elapsed(from, target, initial_velocity, elapsed)
    }

    pub(crate) fn progress_run(self, started_at: Duration) -> MotionProgressRun {
        MotionProgressRun {
            model: self.resolved_model(),
            started_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MotionProgressRun {
    model: MotionModel,
    started_at: Duration,
}

impl MotionProgressRun {
    pub(crate) fn sample_elapsed(&self, now: Duration) -> MotionProgressSample {
        let sample =
            self.model
                .sample_scalar_elapsed(0.0, 1.0, 0.0, now.saturating_sub(self.started_at));
        MotionProgressSample {
            progress: sample.value().clamp(0.0, 1.0),
            complete: self.model.is_immediate() || sample.reached_final_state(),
        }
    }
}

pub(crate) struct MotionProgressSample {
    progress: f32,
    complete: bool,
}

impl MotionProgressSample {
    pub(crate) fn progress(&self) -> f32 {
        self.progress
    }

    pub(crate) fn complete(&self) -> bool {
        self.complete
    }
}
