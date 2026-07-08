//! Renderer-neutral motion policy validation.

use crate::spring::{MotionModel, MotionSpringPhysics};
use std::time::Duration;

/// Maximum routine UI motion duration accepted without a continuity reason.
pub const MOTION_POLICY_MAX_UI_DURATION: Duration = Duration::from_millis(300);

/// Product context for a motion policy validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPolicyContext {
    /// Pointer-coupled drag or resize input.
    PointerDrag,
    /// Keyboard focus, command focus, or high-frequency focus movement.
    KeyboardFocus,
    /// Docking or overlay preview tied to a semantic target.
    VisualAffordancePreview,
    /// Lightweight hover, guide, or presence affordance.
    AffordancePresence,
    /// Committed layout change such as insert, remove, collapse, or expand.
    CommittedLayout,
    /// Continuity motion for retargeted pane, divider, or zoom transitions.
    Continuity,
    /// Decorative motion that does not communicate layout or target semantics.
    Decorative,
}

impl MotionPolicyContext {
    /// Returns whether spatial motion is allowed in this context.
    pub const fn allows_spatial_motion(self) -> bool {
        !matches!(self, Self::PointerDrag | Self::KeyboardFocus)
    }

    /// Returns whether this context may exceed the routine UI duration budget.
    pub const fn allows_extended_duration(self) -> bool {
        matches!(self, Self::Continuity)
    }
}

/// Relationship between a preview sample and the current semantic target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPreviewTargetPolicy {
    /// The motion is not a preview transition.
    NotPreview,
    /// Preview motion stays within the same stable semantic identity.
    SameIdentity,
    /// Preview motion crosses unrelated semantic identities.
    UnrelatedIdentity,
}

/// Deterministic policy issue reported by the motion validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPolicyIssue {
    /// Spatial motion was requested in a high-frequency context.
    SpatialMotionForbidden,
    /// Routine UI motion exceeded the duration budget.
    DurationOverBudget,
    /// Review-facing bounce exceeded the professional UI threshold.
    ExcessiveBounce,
    /// Reduced motion does not preserve final semantic state.
    MissingReducedMotionFinalState,
    /// Preview geometry interpolates across unrelated semantic targets.
    UnrelatedTargetPreviewInterpolation,
}

/// Input to the renderer-neutral motion policy validator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionPolicyInput {
    context: MotionPolicyContext,
    model: MotionModel,
    spatial_motion: bool,
    reduced_motion_final_state: bool,
    preview_target: MotionPreviewTargetPolicy,
    reported_bounce: Option<f32>,
}

impl MotionPolicyInput {
    /// Creates policy input for a context and motion model.
    pub const fn new(context: MotionPolicyContext, model: MotionModel) -> Self {
        Self {
            context,
            model,
            spatial_motion: false,
            reduced_motion_final_state: false,
            preview_target: MotionPreviewTargetPolicy::NotPreview,
            reported_bounce: None,
        }
    }

    /// Returns the policy context.
    pub const fn context(self) -> MotionPolicyContext {
        self.context
    }

    /// Returns the motion model being validated.
    pub const fn model(self) -> MotionModel {
        self.model
    }

    /// Returns whether spatial movement is involved.
    pub const fn spatial_motion(self) -> bool {
        self.spatial_motion
    }

    /// Returns whether reduced motion preserves final semantic state.
    pub const fn reduced_motion_final_state(self) -> bool {
        self.reduced_motion_final_state
    }

    /// Returns the preview target relationship.
    pub const fn preview_target(self) -> MotionPreviewTargetPolicy {
        self.preview_target
    }

    /// Returns an explicit review-facing bounce override.
    pub const fn reported_bounce(self) -> Option<f32> {
        self.reported_bounce
    }

    /// Returns a copy with spatial-motion participation set.
    pub const fn with_spatial_motion(mut self, spatial_motion: bool) -> Self {
        self.spatial_motion = spatial_motion;
        self
    }

    /// Returns a copy with reduced-motion final-state coverage set.
    pub const fn with_reduced_motion_final_state(
        mut self,
        reduced_motion_final_state: bool,
    ) -> Self {
        self.reduced_motion_final_state = reduced_motion_final_state;
        self
    }

    /// Returns a copy with preview target relationship set.
    pub const fn with_preview_target(mut self, preview_target: MotionPreviewTargetPolicy) -> Self {
        self.preview_target = preview_target;
        self
    }

    /// Returns a copy with an explicit review-facing bounce value.
    pub const fn with_reported_bounce(mut self, bounce: f32) -> Self {
        self.reported_bounce = Some(bounce);
        self
    }
}

/// Result of validating motion policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionPolicyReport {
    issues: Vec<MotionPolicyIssue>,
}

impl MotionPolicyReport {
    /// Creates a policy report from issues.
    pub fn new(issues: Vec<MotionPolicyIssue>) -> Self {
        Self { issues }
    }

    /// Returns all policy issues.
    pub fn issues(&self) -> &[MotionPolicyIssue] {
        &self.issues
    }

    /// Returns whether the policy input passed.
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }

    /// Returns whether the report contains an issue.
    pub fn has_issue(&self, issue: MotionPolicyIssue) -> bool {
        self.issues.contains(&issue)
    }
}

/// Validates renderer-neutral motion policy.
pub fn validate_motion_policy(input: MotionPolicyInput) -> MotionPolicyReport {
    let mut issues = Vec::new();

    if input.spatial_motion() && !input.context().allows_spatial_motion() {
        issues.push(MotionPolicyIssue::SpatialMotionForbidden);
    }

    if !input.context().allows_extended_duration()
        && model_review_duration(input.model()) > MOTION_POLICY_MAX_UI_DURATION
    {
        issues.push(MotionPolicyIssue::DurationOverBudget);
    }

    if input
        .reported_bounce()
        .unwrap_or_else(|| model_bounce(input.model()))
        > MotionSpringPhysics::MAX_REVIEWABLE_BOUNCE
    {
        issues.push(MotionPolicyIssue::ExcessiveBounce);
    }

    if !input.reduced_motion_final_state() {
        issues.push(MotionPolicyIssue::MissingReducedMotionFinalState);
    }

    if input.spatial_motion()
        && matches!(
            input.preview_target(),
            MotionPreviewTargetPolicy::UnrelatedIdentity
        )
    {
        issues.push(MotionPolicyIssue::UnrelatedTargetPreviewInterpolation);
    }

    MotionPolicyReport::new(issues)
}

fn model_review_duration(model: MotionModel) -> Duration {
    match model {
        MotionModel::Timeline(spec) => spec.duration().as_duration(),
        MotionModel::Spring(spec) => spec.physics().review_duration(),
    }
}

fn model_bounce(model: MotionModel) -> f32 {
    match model {
        MotionModel::Timeline(_) => 0.0,
        MotionModel::Spring(spec) => spec.physics().bounce(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionDuration, MotionEasing, MotionPreference,
        motion::MotionSpec,
        spring::{MotionModel, MotionSpringSpec},
    };
    use std::time::Duration;

    #[test]
    fn committed_layout_motion_under_budget_passes_policy() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionModel::timeline(MotionSpec::committed_layout(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);

        assert!(validate_motion_policy(input).is_ok());
    }

    #[test]
    fn pointer_drag_spatial_motion_is_rejected() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::PointerDrag,
            MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);

        let report = validate_motion_policy(input);
        assert!(report.has_issue(MotionPolicyIssue::SpatialMotionForbidden));
    }

    #[test]
    fn keyboard_focus_spatial_motion_is_rejected() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::KeyboardFocus,
            MotionModel::timeline(MotionSpec::continuity(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);

        let report = validate_motion_policy(input);
        assert!(report.has_issue(MotionPolicyIssue::SpatialMotionForbidden));
    }

    #[test]
    fn overlong_ui_motion_without_continuity_reason_is_rejected() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionModel::timeline(MotionSpec::new(
                MotionPreference::Animated,
                MotionDuration::Custom(Duration::from_millis(420)),
                MotionEasing::EaseOut,
            )),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);

        let report = validate_motion_policy(input);
        assert!(report.has_issue(MotionPolicyIssue::DurationOverBudget));
    }

    #[test]
    fn excessive_bounce_is_rejected() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_reported_bounce(0.8)
        .with_reduced_motion_final_state(true);

        let report = validate_motion_policy(input);
        assert!(report.has_issue(MotionPolicyIssue::ExcessiveBounce));
    }

    #[test]
    fn unrelated_target_preview_interpolation_is_rejected() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::VisualAffordancePreview,
            MotionModel::spring(MotionSpringSpec::affordance(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_preview_target(MotionPreviewTargetPolicy::UnrelatedIdentity)
        .with_reduced_motion_final_state(true);

        let report = validate_motion_policy(input);
        assert!(report.has_issue(MotionPolicyIssue::UnrelatedTargetPreviewInterpolation));
    }

    #[test]
    fn reduced_motion_final_semantics_without_spatial_motion_pass() {
        let input = MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Reduced)),
        )
        .with_spatial_motion(false)
        .with_reduced_motion_final_state(true);

        assert!(validate_motion_policy(input).is_ok());
    }
}
