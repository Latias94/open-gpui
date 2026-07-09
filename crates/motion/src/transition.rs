//! Root-level motion facade for duration-first adapter-owned animation.

use crate::{
    MotionClockSample, MotionDuration, MotionEasing, MotionFrameDemand, MotionPreference,
    MotionProgressSample, MotionScalarSample, MotionSpringPhysics,
    controller::{MotionExecutionPlan, MotionScalarExecution, MotionScalarExecutionSample},
    frame_host::{
        MotionFrameHost, MotionFrameHostResetReason, MotionFrameHostSample, MotionFrameHostUpdate,
    },
    motion::MotionSpec,
    policy::{MotionPolicyContext, MotionPolicyInput, MotionPolicyReport},
    spring::{MotionModel, MotionSpringSpec},
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
    pub fn spring(
        intent: MotionIntent,
        preference: MotionPreference,
        physics: MotionSpringPhysics,
    ) -> Self {
        Self::from_model(
            intent,
            MotionModel::spring(MotionSpringSpec::from_physics(preference, physics)),
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

    pub(crate) fn resolve_plan(self) -> MotionExecutionPlan {
        MotionExecutionPlan::resolve(self.policy_input())
    }

    /// Returns the policy report for this transition.
    pub fn policy_report(self) -> MotionPolicyReport {
        self.resolve_plan().policy_report().clone()
    }

    /// Samples this transition as a scalar value using explicit elapsed time.
    pub fn sample_scalar_elapsed(
        self,
        from: f32,
        target: f32,
        initial_velocity: f32,
        elapsed: Duration,
    ) -> MotionScalarSample {
        self.resolve_plan()
            .model()
            .sample_scalar_elapsed(from, target, initial_velocity, elapsed)
    }

    /// Starts a normalized 0..1 progress run at controller elapsed time.
    pub fn progress_run(self, started_at: Duration) -> MotionProgressRun {
        MotionProgressRun::start(self, started_at)
    }

    /// Starts a scalar run at controller elapsed time.
    pub fn scalar_run(
        self,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> MotionScalarRun {
        MotionScalarRun::start(self, from, target, initial_velocity, started_at)
    }
}

/// Sample returned by [`MotionScalarRun`].
pub type MotionScalarRunSample = MotionScalarExecutionSample;

/// Duration-first scalar run facade.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarRun {
    transition: MotionTransition,
    execution: MotionScalarExecution,
}

impl MotionScalarRun {
    /// Starts a scalar run from a facade transition.
    pub fn start(
        transition: MotionTransition,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> Self {
        let execution = MotionScalarExecution::start(
            transition.resolve_plan(),
            from,
            target,
            initial_velocity,
            started_at,
        );
        Self {
            transition,
            execution,
        }
    }

    /// Returns the facade transition.
    pub const fn transition(&self) -> MotionTransition {
        self.transition
    }

    /// Returns the policy report produced when this run started.
    pub const fn policy_report(&self) -> &MotionPolicyReport {
        self.execution.policy_report()
    }

    /// Samples the run at controller elapsed time.
    pub fn sample_elapsed(&self, elapsed: Duration) -> MotionScalarRunSample {
        self.execution.sample_at(elapsed)
    }

    /// Samples the run from a clamped adapter clock sample.
    pub fn sample_clock(&self, clock: MotionClockSample) -> MotionScalarRunSample {
        self.execution.sample_clock(clock)
    }
}

/// Duration-first normalized 0..1 progress run facade.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionProgressRun {
    transition: MotionTransition,
    execution: MotionScalarExecution,
}

impl MotionProgressRun {
    /// Starts a progress run from a facade transition.
    pub fn start(transition: MotionTransition, started_at: Duration) -> Self {
        let execution =
            MotionScalarExecution::start(transition.resolve_plan(), 0.0, 1.0, 0.0, started_at);
        Self {
            transition,
            execution,
        }
    }

    /// Returns the facade transition.
    pub const fn transition(&self) -> MotionTransition {
        self.transition
    }

    /// Returns the policy report produced when this run started.
    pub const fn policy_report(&self) -> &MotionPolicyReport {
        self.execution.policy_report()
    }

    /// Samples the progress run at controller elapsed time.
    pub fn sample_elapsed(&self, elapsed: Duration) -> MotionProgressSample {
        MotionProgressSample::new(self.execution.sample_at(elapsed))
    }

    /// Samples the progress run from a clamped adapter clock sample.
    pub fn sample_clock(&self, clock: MotionClockSample) -> MotionProgressSample {
        MotionProgressSample::new(self.execution.sample_clock(clock))
    }
}

/// Product-level reason an adapter starts a new local motion frame epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFrameResetReason {
    /// The adapter replaced the target of an existing motion from the current sampled value.
    Retarget,
    /// The adapter cancelled an active motion without publishing the semantic final state.
    Cancel,
    /// The adapter forced a motion to its semantic final state.
    Finish,
    /// The adapter removed terminal tracks or presentation state after idle demand was observed.
    PruneTerminal,
    /// The adapter replaced the stable motion identity, such as a row key, pane id, or panel set.
    MotionIdentityChanged,
}

impl MotionFrameResetReason {
    const fn into_host(self) -> MotionFrameHostResetReason {
        match self {
            Self::Retarget => MotionFrameHostResetReason::Retarget,
            Self::Cancel => MotionFrameHostResetReason::Cancel,
            Self::Finish => MotionFrameHostResetReason::Finish,
            Self::PruneTerminal => MotionFrameHostResetReason::PruneTerminal,
            Self::MotionIdentityChanged => MotionFrameHostResetReason::MotionIdentityChanged,
        }
    }

    const fn from_host(reason: MotionFrameHostResetReason) -> Self {
        match reason {
            MotionFrameHostResetReason::Retarget => Self::Retarget,
            MotionFrameHostResetReason::Cancel => Self::Cancel,
            MotionFrameHostResetReason::Finish => Self::Finish,
            MotionFrameHostResetReason::PruneTerminal => Self::PruneTerminal,
            MotionFrameHostResetReason::MotionIdentityChanged => Self::MotionIdentityChanged,
        }
    }
}

/// Adapter decision produced after a frame driver observes motion demand.
#[must_use = "adapter frame updates must be translated into the owner's frame request API"]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameDriverUpdate {
    update: MotionFrameHostUpdate,
}

impl MotionFrameDriverUpdate {
    /// Returns the frame demand that produced this update.
    pub const fn frame_demand(self) -> MotionFrameDemand {
        self.update.frame_demand()
    }

    /// Returns whether the adapter should request another frame.
    pub const fn should_request_frame(self) -> bool {
        self.update.should_request_frame()
    }

    /// Returns the driver's cumulative requested-frame count after this update.
    pub const fn requested_frames(self) -> u64 {
        self.update.requested_frames()
    }
}

impl From<MotionFrameHostUpdate> for MotionFrameDriverUpdate {
    fn from(update: MotionFrameHostUpdate) -> Self {
        Self { update }
    }
}

/// Value sampled through a frame driver plus the adapter decision.
#[must_use = "frame driver samples include the adapter's next-frame decision"]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionFrameDriverSample<T> {
    sample: MotionFrameHostSample<T>,
}

impl<T> MotionFrameDriverSample<T> {
    /// Returns the sampled value.
    pub const fn value(&self) -> &T {
        self.sample.value()
    }

    /// Consumes the sample and returns the sampled value.
    pub fn into_value(self) -> T {
        self.sample.into_value()
    }

    /// Returns the clamped clock used for sampling.
    pub const fn clock(&self) -> MotionClockSample {
        self.sample.clock()
    }

    /// Returns the driver update produced after sampling.
    pub const fn update(&self) -> MotionFrameDriverUpdate {
        MotionFrameDriverUpdate {
            update: self.sample.update(),
        }
    }

    /// Returns the frame demand that produced this sample.
    pub const fn frame_demand(&self) -> MotionFrameDemand {
        self.sample.frame_demand()
    }

    /// Returns whether the adapter should request another frame.
    pub const fn should_request_frame(&self) -> bool {
        self.sample.should_request_frame()
    }
}

impl<T> From<MotionFrameHostSample<T>> for MotionFrameDriverSample<T> {
    fn from(sample: MotionFrameHostSample<T>) -> Self {
        Self { sample }
    }
}

/// Adapter-owned frame driver facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameDriver {
    host: MotionFrameHost,
}

impl MotionFrameDriver {
    /// Creates an idle frame driver.
    pub const fn new() -> Self {
        Self {
            host: MotionFrameHost::new(),
        }
    }

    /// Returns the last clamped elapsed time observed by this driver.
    pub const fn last_elapsed(&self) -> Duration {
        self.host.last_elapsed()
    }

    /// Returns the latest observed frame demand.
    pub const fn last_frame_demand(&self) -> MotionFrameDemand {
        self.host.last_frame_demand()
    }

    /// Returns the reason the current local motion epoch was last reset.
    pub fn last_reset_reason(&self) -> Option<MotionFrameResetReason> {
        self.host
            .last_reset_reason()
            .map(MotionFrameResetReason::from_host)
    }

    /// Returns how many frame requests this driver has asked the adapter to issue.
    pub const fn requested_frames(&self) -> u64 {
        self.host.requested_frames()
    }

    /// Resets elapsed time and demand state for a new local motion epoch.
    pub const fn reset(&mut self, reason: MotionFrameResetReason) {
        self.host.reset(reason.into_host());
    }

    /// Observes a frame demand and returns the adapter decision for this render pass.
    pub fn observe(&mut self, frame_demand: MotionFrameDemand) -> MotionFrameDriverUpdate {
        self.host.observe(frame_demand).into()
    }

    /// Combines many frame demands and returns the adapter decision for this render pass.
    pub fn observe_all(
        &mut self,
        demands: impl IntoIterator<Item = MotionFrameDemand>,
    ) -> MotionFrameDriverUpdate {
        self.host.observe_all(demands).into()
    }

    /// Samples motion from explicit adapter elapsed time and records the frame demand.
    pub fn sample_elapsed<T>(
        &mut self,
        requested_elapsed: Duration,
        sample: impl FnOnce(MotionClockSample) -> (T, MotionFrameDemand),
    ) -> MotionFrameDriverSample<T> {
        self.host.sample_elapsed(requested_elapsed, sample).into()
    }
}

impl Default for MotionFrameDriver {
    fn default() -> Self {
        Self::new()
    }
}
