//! Renderer-neutral motion controller contracts.

use crate::motion_spring::{MotionModel, MotionScalarSample};
use crate::motion_value::MotionValue;
use crate::{
    MotionPolicyInput, MotionPolicyReport, MotionRunState, MotionSpec, validate_motion_policy,
};
use std::time::{Duration, Instant};

/// Renderer-neutral frame demand returned by motion controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFrameDemand {
    /// No more animation frames are required.
    Idle,
    /// At least one track is active and the adapter should request another frame for the reason.
    NeedsFrame(MotionFrameReason),
}

/// Minimal reason vocabulary for adapter-owned frame requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFrameReason {
    /// The adapter should sample motion and render the updated presentation state.
    UpdateRender,
}

impl MotionFrameDemand {
    /// Returns whether another frame should be requested by the adapter.
    pub const fn needs_frame(self) -> bool {
        matches!(self, Self::NeedsFrame(_))
    }

    /// Returns the reason another frame is needed.
    pub const fn reason(self) -> Option<MotionFrameReason> {
        match self {
            Self::Idle => None,
            Self::NeedsFrame(reason) => Some(reason),
        }
    }

    fn from_active(active: bool) -> Self {
        if active {
            Self::NeedsFrame(MotionFrameReason::UpdateRender)
        } else {
            Self::Idle
        }
    }

    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::NeedsFrame(reason), _) | (_, Self::NeedsFrame(reason)) => {
                Self::NeedsFrame(reason)
            }
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }
}

/// Policy-resolved execution state for a motion run before adapter sampling begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionExecutionState {
    /// The run should publish its final semantic state without requesting frames.
    Immediate,
    /// The run may sample over time and request adapter-owned frames.
    Scheduled,
}

impl MotionExecutionState {
    /// Returns whether the run completes immediately.
    pub const fn is_immediate(self) -> bool {
        matches!(self, Self::Immediate)
    }

    /// Returns whether the run should be sampled over time.
    pub const fn is_scheduled(self) -> bool {
        matches!(self, Self::Scheduled)
    }
}

/// Renderer-neutral policy result used to start motion from a single owner.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionExecutionPlan {
    model: MotionModel,
    policy_report: MotionPolicyReport,
    state: MotionExecutionState,
}

impl MotionExecutionPlan {
    /// Resolves a requested model through motion policy, falling back to immediate motion on
    /// policy failure.
    pub fn resolve(input: MotionPolicyInput) -> Self {
        let requested_model = input.model();
        let policy_report = validate_motion_policy(input);
        let model = if policy_report.is_ok() {
            requested_model
        } else {
            MotionModel::timeline(MotionSpec::immediate())
        };
        let state = if model.is_immediate() {
            MotionExecutionState::Immediate
        } else {
            MotionExecutionState::Scheduled
        };
        Self {
            model,
            policy_report,
            state,
        }
    }

    /// Returns the model that should execute after policy resolution.
    pub const fn model(&self) -> MotionModel {
        self.model
    }

    /// Returns the policy report produced for the requested model.
    pub const fn policy_report(&self) -> &MotionPolicyReport {
        &self.policy_report
    }

    /// Returns the policy-resolved execution state.
    pub const fn state(&self) -> MotionExecutionState {
        self.state
    }

    /// Returns whether the run should complete immediately.
    pub const fn is_immediate(&self) -> bool {
        self.state.is_immediate()
    }

    /// Consumes the plan and returns its parts.
    pub fn into_parts(self) -> (MotionModel, MotionPolicyReport, MotionExecutionState) {
        (self.model, self.policy_report, self.state)
    }
}

/// One scalar motion track sampled by deterministic elapsed time.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarTrack {
    model: MotionModel,
    value: MotionValue,
    target: f32,
    initial_velocity: f32,
    started_at: Duration,
    cancelled_at: Option<Duration>,
}

impl MotionScalarTrack {
    /// Starts a scalar track at the provided controller time.
    pub fn start(
        model: MotionModel,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> Self {
        Self {
            model,
            value: MotionValue::new(from),
            target,
            initial_velocity,
            started_at,
            cancelled_at: None,
        }
    }

    /// Creates an immediate scalar track at a fixed value.
    pub fn immediate(value: f32, started_at: Duration) -> Self {
        Self::start(
            MotionModel::timeline(MotionSpec::immediate()),
            value,
            value,
            0.0,
            started_at,
        )
    }

    /// Returns the motion model.
    pub const fn model(&self) -> MotionModel {
        self.model
    }

    /// Returns the source value.
    pub const fn from(&self) -> f32 {
        self.value.current()
    }

    /// Returns the target value.
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// Returns the initial velocity.
    pub const fn initial_velocity(&self) -> f32 {
        self.initial_velocity
    }

    /// Returns the controller time at which the track started.
    pub const fn started_at(&self) -> Duration {
        self.started_at
    }

    /// Returns the controller time at which the track was cancelled.
    pub const fn cancelled_at(&self) -> Option<Duration> {
        self.cancelled_at
    }

    /// Cancels the track at the provided controller time.
    pub fn cancel_at(&mut self, cancelled_at: Duration) {
        self.cancelled_at = Some(cancelled_at);
    }

    /// Retargets the track from its sampled value and velocity.
    pub fn retarget(&self, model: MotionModel, target: f32, now: Duration) -> Self {
        let sample = self.sample_at(now);
        Self::start(model, sample.value(), target, sample.velocity(), now)
    }

    /// Samples the track at the provided controller time.
    pub fn sample_at(&self, now: Duration) -> MotionScalarSample {
        let effective_now = self.cancelled_at.unwrap_or(now);
        let elapsed = effective_now.saturating_sub(self.started_at);
        let mut sample = self.model.sample_scalar_elapsed(
            self.value.current(),
            self.target,
            self.initial_velocity,
            elapsed,
        );
        if self.cancelled_at.is_some() && sample.state().is_active() {
            sample = MotionScalarSample::new(
                MotionRunState::Cancelled,
                sample.elapsed(),
                sample.value(),
                sample.velocity(),
                sample.target(),
            );
        }
        sample
    }

    /// Returns whether this track needs another adapter-owned frame.
    pub fn frame_demand_at(&self, now: Duration) -> MotionFrameDemand {
        MotionFrameDemand::from_active(self.sample_at(now).is_active())
    }
}

/// Sample from a policy-resolved scalar execution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionScalarExecutionSample {
    sample: MotionScalarSample,
    complete: bool,
    frame_demand: MotionFrameDemand,
}

impl MotionScalarExecutionSample {
    /// Creates a scalar execution sample from explicit values.
    pub const fn new(
        sample: MotionScalarSample,
        complete: bool,
        frame_demand: MotionFrameDemand,
    ) -> Self {
        Self {
            sample,
            complete,
            frame_demand,
        }
    }

    /// Returns the underlying scalar sample.
    pub const fn scalar_sample(self) -> MotionScalarSample {
        self.sample
    }

    /// Returns the sampled scalar value.
    pub const fn value(self) -> f32 {
        self.sample.value()
    }

    /// Returns whether the run has reached the semantic completion state.
    pub const fn complete(self) -> bool {
        self.complete
    }

    /// Returns whether the adapter should request another frame.
    pub const fn frame_demand(self) -> MotionFrameDemand {
        self.frame_demand
    }
}

/// A single scalar track plus its policy-resolved execution metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarExecution {
    plan: MotionExecutionPlan,
    track: MotionScalarTrack,
}

impl MotionScalarExecution {
    /// Starts a scalar execution from an already resolved motion plan.
    pub fn start(
        plan: MotionExecutionPlan,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> Self {
        let track =
            MotionScalarTrack::start(plan.model(), from, target, initial_velocity, started_at);
        Self { plan, track }
    }

    /// Resolves policy input and starts a scalar execution.
    pub fn start_resolved(
        input: MotionPolicyInput,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> Self {
        Self::start(
            MotionExecutionPlan::resolve(input),
            from,
            target,
            initial_velocity,
            started_at,
        )
    }

    /// Returns the resolved execution plan.
    pub const fn plan(&self) -> &MotionExecutionPlan {
        &self.plan
    }

    /// Returns the underlying scalar track.
    pub const fn track(&self) -> &MotionScalarTrack {
        &self.track
    }

    /// Returns the model that should execute after policy resolution.
    pub const fn model(&self) -> MotionModel {
        self.plan.model()
    }

    /// Returns the policy report produced for the requested model.
    pub const fn policy_report(&self) -> &MotionPolicyReport {
        self.plan.policy_report()
    }

    /// Returns the policy-resolved execution state.
    pub const fn state(&self) -> MotionExecutionState {
        self.plan.state()
    }

    /// Samples the execution at deterministic elapsed time.
    pub fn sample_at(&self, now: Duration) -> MotionScalarExecutionSample {
        let sample = self.track.sample_at(now);
        let complete = self.plan.is_immediate() || sample.reached_final_state();
        let frame_demand = if complete {
            MotionFrameDemand::Idle
        } else {
            MotionFrameDemand::from_active(sample.is_active())
        };
        MotionScalarExecutionSample::new(sample, complete, frame_demand)
    }

    /// Samples the execution from adapter instants while keeping deterministic elapsed-time
    /// semantics in the controller layer.
    pub fn sample_since(&self, started_at: Instant, now: Instant) -> MotionScalarExecutionSample {
        self.sample_at(now.saturating_duration_since(started_at))
    }
}

/// A sampled keyed scalar motion track.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarTrackSample<K> {
    key: K,
    sample: MotionScalarSample,
}

impl<K> MotionScalarTrackSample<K> {
    /// Creates a keyed track sample.
    pub const fn new(key: K, sample: MotionScalarSample) -> Self {
        Self { key, sample }
    }

    /// Returns the track key.
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the scalar motion sample.
    pub const fn sample(&self) -> MotionScalarSample {
        self.sample
    }
}

/// A grouped controller sample.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarControllerSample<K> {
    tracks: Vec<MotionScalarTrackSample<K>>,
    frame_demand: MotionFrameDemand,
}

impl<K> MotionScalarControllerSample<K> {
    /// Creates a grouped sample.
    pub fn new(tracks: Vec<MotionScalarTrackSample<K>>, frame_demand: MotionFrameDemand) -> Self {
        Self {
            tracks,
            frame_demand,
        }
    }

    /// Returns sampled tracks in controller order.
    pub fn tracks(&self) -> &[MotionScalarTrackSample<K>] {
        &self.tracks
    }

    /// Returns the grouped frame demand.
    pub const fn frame_demand(&self) -> MotionFrameDemand {
        self.frame_demand
    }

    /// Returns whether all grouped tracks are terminal for adapter frame scheduling.
    pub const fn complete(&self) -> bool {
        !self.frame_demand.needs_frame()
    }
}

impl<K: PartialEq> MotionScalarControllerSample<K> {
    /// Returns the sample for a key.
    pub fn track(&self, key: &K) -> Option<&MotionScalarTrackSample<K>> {
        self.tracks.iter().find(|track| track.key() == key)
    }
}

/// A small keyed scalar motion controller.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarController<K> {
    tracks: Vec<(K, MotionScalarTrack)>,
}

impl<K> Default for MotionScalarController<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> MotionScalarController<K> {
    /// Creates an empty scalar motion controller.
    pub const fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    /// Returns all registered tracks in insertion order.
    pub fn tracks(&self) -> &[(K, MotionScalarTrack)] {
        &self.tracks
    }
}

impl<K: PartialEq> MotionScalarController<K> {
    /// Starts or replaces a keyed scalar track.
    pub fn start(
        &mut self,
        key: K,
        model: MotionModel,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) {
        let track = MotionScalarTrack::start(model, from, target, initial_velocity, started_at);
        if let Some((_, existing)) = self
            .tracks
            .iter_mut()
            .find(|(track_key, _)| track_key == &key)
        {
            *existing = track;
        } else {
            self.tracks.push((key, track));
        }
    }

    /// Sets or replaces a keyed scalar track with an immediate fixed value.
    pub fn set_immediate(&mut self, key: K, value: f32, now: Duration) {
        let track = MotionScalarTrack::immediate(value, now);
        if let Some((_, existing)) = self
            .tracks
            .iter_mut()
            .find(|(track_key, _)| track_key == &key)
        {
            *existing = track;
        } else {
            self.tracks.push((key, track));
        }
    }

    /// Retargets an existing keyed track from its sampled value and velocity.
    pub fn retarget(&mut self, key: K, model: MotionModel, target: f32, now: Duration) {
        if let Some((_, existing)) = self
            .tracks
            .iter_mut()
            .find(|(track_key, _)| track_key == &key)
        {
            *existing = existing.retarget(model, target, now);
        } else {
            self.start(key, model, target, target, 0.0, now);
        }
    }

    /// Cancels an existing keyed track at the provided controller time.
    pub fn cancel(&mut self, key: &K, now: Duration) {
        if let Some((_, existing)) = self
            .tracks
            .iter_mut()
            .find(|(track_key, _)| track_key == key)
        {
            existing.cancel_at(now);
        }
    }

    /// Returns the frame demand at the provided controller time.
    pub fn frame_demand_at(&self, now: Duration) -> MotionFrameDemand {
        self.tracks
            .iter()
            .map(|(_, track)| track.frame_demand_at(now))
            .fold(MotionFrameDemand::Idle, MotionFrameDemand::combine)
    }
}

impl<K: Clone> MotionScalarController<K> {
    /// Samples all tracks at the provided controller time.
    pub fn sample_at(&self, now: Duration) -> MotionScalarControllerSample<K> {
        let tracks = self
            .tracks
            .iter()
            .map(|(key, track)| MotionScalarTrackSample::new(key.clone(), track.sample_at(now)))
            .collect::<Vec<_>>();
        let frame_demand = tracks
            .iter()
            .map(|track| MotionFrameDemand::from_active(track.sample().is_active()))
            .fold(MotionFrameDemand::Idle, MotionFrameDemand::combine);
        MotionScalarControllerSample::new(tracks, frame_demand)
    }

    /// Samples all tracks from adapter instants while keeping deterministic elapsed-time
    /// semantics in the controller layer.
    pub fn sample_since(
        &self,
        started_at: Instant,
        now: Instant,
    ) -> MotionScalarControllerSample<K> {
        self.sample_at(now.saturating_duration_since(started_at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionDuration, MotionEasing, MotionModel, MotionPolicyContext, MotionPreference,
        MotionRunState, MotionSpec, MotionSpringSpec,
    };
    use std::time::Duration;

    #[test]
    fn grouped_tracks_report_frame_demand_until_every_track_completes() {
        let mut controller = MotionScalarController::new();
        let model = MotionModel::timeline(MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(100)),
            MotionEasing::Linear,
        ));

        controller.start("left", model, 0.0, 1.0, 0.0, Duration::ZERO);
        controller.start("right", model, 1.0, 0.0, 0.0, Duration::ZERO);

        let active = controller.sample_at(Duration::from_millis(50));
        assert!(active.frame_demand().needs_frame());
        assert_eq!(
            active.frame_demand().reason(),
            Some(MotionFrameReason::UpdateRender)
        );
        assert_eq!(active.tracks().len(), 2);
        assert!(
            active
                .tracks()
                .iter()
                .all(|track| track.sample().is_active())
        );

        let complete = controller.sample_at(Duration::from_millis(120));
        assert!(!complete.frame_demand().needs_frame());
        assert!(
            complete
                .tracks()
                .iter()
                .all(|track| track.sample().reached_final_state())
        );
        assert!(complete.complete());
    }

    #[test]
    fn retarget_preserves_sampled_value_and_velocity_for_one_track() {
        let mut controller = MotionScalarController::new();
        let model = MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated));

        controller.start("pane", model, 0.0, 1.0, 0.0, Duration::ZERO);
        let before = controller.sample_at(Duration::from_millis(80));
        let sampled = before.track(&"pane").expect("pane track").sample();

        controller.retarget("pane", model, 2.0, Duration::from_millis(80));
        let after = controller.sample_at(Duration::from_millis(80));
        let retargeted = after.track(&"pane").expect("pane track").sample();

        assert_eq!(retargeted.value(), sampled.value());
        assert_eq!(retargeted.velocity(), sampled.velocity());
        assert_eq!(retargeted.target(), 2.0);
        assert!(after.frame_demand().needs_frame());
        assert_eq!(
            after.frame_demand().reason(),
            Some(MotionFrameReason::UpdateRender)
        );
    }

    #[test]
    fn cancelling_track_is_terminal_without_reaching_final_state() {
        let mut track = MotionScalarTrack::start(
            MotionModel::timeline(MotionSpec::new(
                MotionPreference::Animated,
                MotionDuration::Custom(Duration::from_millis(200)),
                MotionEasing::Linear,
            )),
            0.0,
            1.0,
            0.0,
            Duration::ZERO,
        );

        track.cancel_at(Duration::from_millis(40));
        let sample = track.sample_at(Duration::from_millis(100));

        assert_eq!(sample.state(), MotionRunState::Cancelled);
        assert!(!sample.reached_final_state());
        assert!(
            !track
                .frame_demand_at(Duration::from_millis(100))
                .needs_frame()
        );
    }

    #[test]
    fn immediate_track_never_requests_a_frame() {
        let track = MotionScalarTrack::immediate(0.75, Duration::from_millis(10));

        let sample = track.sample_at(Duration::from_millis(10));
        assert_eq!(sample.state(), MotionRunState::Immediate);
        assert_eq!(sample.value(), 0.75);
        assert!(
            !track
                .frame_demand_at(Duration::from_millis(10))
                .needs_frame()
        );
    }

    #[test]
    fn execution_plan_downgrades_policy_failures_to_immediate_motion() {
        let requested_model = MotionModel::timeline(MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(900)),
            MotionEasing::Linear,
        ));
        let plan = MotionExecutionPlan::resolve(
            MotionPolicyInput::new(MotionPolicyContext::CommittedLayout, requested_model)
                .with_spatial_motion(true)
                .with_reduced_motion_final_state(true),
        );

        assert!(!plan.policy_report().is_ok());
        assert!(plan.is_immediate());
        assert!(plan.model().is_immediate());
    }

    #[test]
    fn scalar_execution_reports_completion_and_frame_demand_from_one_sample() {
        let execution = MotionScalarExecution::start_resolved(
            MotionPolicyInput::new(
                MotionPolicyContext::CommittedLayout,
                MotionModel::timeline(MotionSpec::new(
                    MotionPreference::Animated,
                    MotionDuration::Custom(Duration::from_millis(100)),
                    MotionEasing::Linear,
                )),
            )
            .with_spatial_motion(true)
            .with_reduced_motion_final_state(true),
            0.0,
            1.0,
            0.0,
            Duration::ZERO,
        );

        let midpoint = execution.sample_at(Duration::from_millis(50));
        assert_eq!(midpoint.value(), 0.5);
        assert!(!midpoint.complete());
        assert!(midpoint.frame_demand().needs_frame());

        let complete = execution.sample_at(Duration::from_millis(120));
        assert_eq!(complete.value(), 1.0);
        assert!(complete.complete());
        assert!(!complete.frame_demand().needs_frame());
    }
}
