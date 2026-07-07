//! Renderer-neutral motion controller contracts.

use crate::spring::{MotionModel, MotionScalarSample};
use crate::value::MotionValue;
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

    /// Combines two frame demands into one adapter-owned frame request.
    pub const fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::NeedsFrame(reason), _) | (_, Self::NeedsFrame(reason)) => {
                Self::NeedsFrame(reason)
            }
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }

    /// Combines many frame demands into one adapter-owned frame request.
    pub fn combine_all(demands: impl IntoIterator<Item = Self>) -> Self {
        demands.into_iter().fold(Self::Idle, Self::combine)
    }
}

/// Adapter clock sample mapped into deterministic controller elapsed time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionClockSample {
    elapsed: Duration,
    delta: Duration,
    clamped: bool,
}

impl MotionClockSample {
    /// A zero elapsed-time clock sample.
    pub const ZERO: Self = Self {
        elapsed: Duration::ZERO,
        delta: Duration::ZERO,
        clamped: false,
    };

    /// Creates a clock sample from previous and requested elapsed times.
    ///
    /// Non-monotonic elapsed time is clamped to the previous elapsed time. This keeps controller
    /// sampling deterministic and avoids negative deltas for adapters whose frame clock moves
    /// backwards or is restored from stale state.
    pub fn from_elapsed(previous_elapsed: Duration, requested_elapsed: Duration) -> Self {
        if requested_elapsed < previous_elapsed {
            Self {
                elapsed: previous_elapsed,
                delta: Duration::ZERO,
                clamped: true,
            }
        } else {
            Self {
                elapsed: requested_elapsed,
                delta: requested_elapsed - previous_elapsed,
                clamped: false,
            }
        }
    }

    /// Creates a clock sample from a start instant and current adapter instant.
    pub fn from_instant(started_at: Instant, now: Instant) -> Self {
        Self::from_elapsed(Duration::ZERO, now.saturating_duration_since(started_at))
    }

    /// Creates a clock sample from adapter instants and clamps non-monotonic elapsed time.
    pub fn from_instants(started_at: Instant, previous_now: Instant, now: Instant) -> Self {
        Self::from_elapsed(
            previous_now.saturating_duration_since(started_at),
            now.saturating_duration_since(started_at),
        )
    }

    /// Returns clamped controller elapsed time.
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns elapsed-time delta since the previous sample.
    pub const fn delta(self) -> Duration {
        self.delta
    }

    /// Returns whether requested elapsed time was clamped.
    pub const fn clamped(self) -> bool {
        self.clamped
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
    finished_at: Option<Duration>,
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
            finished_at: None,
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

    /// Returns the controller time at which the track was explicitly finished.
    pub const fn finished_at(&self) -> Option<Duration> {
        self.finished_at
    }

    /// Cancels the track at the provided controller time.
    pub fn cancel_at(&mut self, cancelled_at: Duration) {
        if self.finished_at.is_none() {
            self.cancelled_at = Some(cancelled_at);
        }
    }

    /// Finishes the track at the provided controller time and publishes the target value.
    pub fn finish_at(&mut self, finished_at: Duration) {
        self.finished_at = Some(finished_at);
        self.cancelled_at = None;
    }

    /// Retargets the track from its sampled value and velocity.
    pub fn retarget(&self, model: MotionModel, target: f32, now: Duration) -> Self {
        let sample = self.sample_at(now);
        Self::start(model, sample.value(), target, sample.velocity(), now)
    }

    /// Samples the track at the provided controller time.
    pub fn sample_at(&self, now: Duration) -> MotionScalarSample {
        if let Some(finished_at) = self.finished_at {
            return MotionScalarSample::new(
                MotionRunState::Completed,
                finished_at.saturating_sub(self.started_at),
                self.target,
                0.0,
                self.target,
            );
        }

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
    const fn new(
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

/// Sample from a normalized 0..1 progress execution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProgressSample {
    sample: MotionScalarExecutionSample,
}

impl MotionProgressSample {
    const fn new(sample: MotionScalarExecutionSample) -> Self {
        Self { sample }
    }

    /// Returns the underlying scalar execution sample.
    pub const fn scalar_sample(self) -> MotionScalarExecutionSample {
        self.sample
    }

    /// Returns the clamped normalized progress.
    pub fn progress(self) -> f32 {
        self.sample.value().clamp(0.0, 1.0)
    }

    /// Returns whether the progress run reached its semantic completion state.
    pub const fn complete(self) -> bool {
        self.sample.complete()
    }

    /// Returns whether the adapter should request another frame.
    pub const fn frame_demand(self) -> MotionFrameDemand {
        self.sample.frame_demand()
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

    /// Samples the execution at a deterministic adapter clock sample.
    pub fn sample_clock(&self, clock: MotionClockSample) -> MotionScalarExecutionSample {
        self.sample_at(clock.elapsed())
    }

    /// Samples the execution from adapter instants while keeping deterministic elapsed-time
    /// semantics in the controller layer.
    pub fn sample_since(&self, started_at: Instant, now: Instant) -> MotionScalarExecutionSample {
        self.sample_at(now.saturating_duration_since(started_at))
    }
}

/// A policy-resolved normalized 0..1 progress run.
///
/// This is the renderer-neutral lifecycle primitive for adapters that need one progress value to
/// drive their own layout, geometry, or paint projection. It deliberately does not schedule frames
/// itself; callers translate the returned [`MotionFrameDemand`] through their adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionProgressExecution {
    execution: MotionScalarExecution,
    started_at: Instant,
}

impl MotionProgressExecution {
    /// Starts a normalized progress run from an already resolved motion plan.
    pub fn start(plan: MotionExecutionPlan, started_at: Instant) -> Self {
        Self {
            execution: MotionScalarExecution::start(plan, 0.0, 1.0, 0.0, Duration::ZERO),
            started_at,
        }
    }

    /// Resolves policy input and starts a normalized progress run.
    pub fn start_resolved(input: MotionPolicyInput, started_at: Instant) -> Self {
        Self::start(MotionExecutionPlan::resolve(input), started_at)
    }

    /// Returns the underlying scalar execution.
    pub const fn scalar_execution(&self) -> &MotionScalarExecution {
        &self.execution
    }

    /// Returns the resolved execution plan.
    pub const fn plan(&self) -> &MotionExecutionPlan {
        self.execution.plan()
    }

    /// Returns the model that should execute after policy resolution.
    pub const fn model(&self) -> MotionModel {
        self.execution.model()
    }

    /// Returns the policy report produced for the requested model.
    pub const fn policy_report(&self) -> &MotionPolicyReport {
        self.execution.policy_report()
    }

    /// Returns the policy-resolved execution state.
    pub const fn state(&self) -> MotionExecutionState {
        self.execution.state()
    }

    /// Returns the adapter instant at which this progress run started.
    pub const fn started_at(&self) -> Instant {
        self.started_at
    }

    /// Samples the progress run at deterministic elapsed time.
    pub fn sample_at(&self, now: Duration) -> MotionProgressSample {
        MotionProgressSample::new(self.execution.sample_at(now))
    }

    /// Samples the progress run at a deterministic adapter clock sample.
    pub fn sample_clock(&self, clock: MotionClockSample) -> MotionProgressSample {
        MotionProgressSample::new(self.execution.sample_clock(clock))
    }

    /// Samples the progress run from adapter instants while keeping deterministic elapsed-time
    /// semantics in the controller layer.
    pub fn sample_since(&self, now: Instant) -> MotionProgressSample {
        MotionProgressSample::new(self.execution.sample_since(self.started_at, now))
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

    /// Finishes an existing keyed track at the provided controller time.
    pub fn finish(&mut self, key: &K, now: Duration) {
        if let Some((_, existing)) = self
            .tracks
            .iter_mut()
            .find(|(track_key, _)| track_key == key)
        {
            existing.finish_at(now);
        }
    }

    /// Returns the frame demand at the provided controller time.
    pub fn frame_demand_at(&self, now: Duration) -> MotionFrameDemand {
        MotionFrameDemand::combine_all(
            self.tracks
                .iter()
                .map(|(_, track)| track.frame_demand_at(now)),
        )
    }
}

impl<K> MotionScalarController<K> {
    /// Removes terminal tracks and returns the number of pruned entries.
    pub fn prune_terminal_at(&mut self, now: Duration) -> usize {
        let before = self.tracks.len();
        self.tracks
            .retain(|(_, track)| track.frame_demand_at(now).needs_frame());
        before - self.tracks.len()
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
        let frame_demand = MotionFrameDemand::combine_all(
            tracks
                .iter()
                .map(|track| MotionFrameDemand::from_active(track.sample().is_active())),
        );
        MotionScalarControllerSample::new(tracks, frame_demand)
    }

    /// Samples all tracks at a deterministic adapter clock sample.
    pub fn sample_clock(&self, clock: MotionClockSample) -> MotionScalarControllerSample<K> {
        self.sample_at(clock.elapsed())
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
    fn frame_demand_combines_idle_and_active_with_stable_reason() {
        let active = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);

        assert_eq!(MotionFrameDemand::Idle.combine(active), active);
        assert_eq!(active.combine(MotionFrameDemand::Idle), active);
        assert_eq!(
            MotionFrameDemand::combine_all([
                MotionFrameDemand::Idle,
                active,
                MotionFrameDemand::Idle,
            ]),
            active
        );
    }

    #[test]
    fn clock_sample_clamps_non_monotonic_elapsed_time() {
        let sample =
            MotionClockSample::from_elapsed(Duration::from_millis(40), Duration::from_millis(15));

        assert_eq!(sample.elapsed(), Duration::from_millis(40));
        assert_eq!(sample.delta(), Duration::ZERO);
        assert!(sample.clamped());
    }

    #[test]
    fn finishing_track_jumps_to_target_and_stops_frame_demand() {
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

        track.finish_at(Duration::from_millis(40));
        let sample = track.sample_at(Duration::from_millis(50));

        assert_eq!(sample.state(), MotionRunState::Completed);
        assert_eq!(sample.value(), 1.0);
        assert!(sample.reached_final_state());
        assert!(
            !track
                .frame_demand_at(Duration::from_millis(50))
                .needs_frame()
        );
    }

    #[test]
    fn controller_prunes_terminal_tracks_after_cancel_and_finish() {
        let mut controller = MotionScalarController::new();
        let model = MotionModel::timeline(MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        ));

        controller.start("cancelled", model, 0.0, 1.0, 0.0, Duration::ZERO);
        controller.start("finished", model, 0.0, 1.0, 0.0, Duration::ZERO);
        controller.start("active", model, 0.0, 1.0, 0.0, Duration::ZERO);
        controller.cancel(&"cancelled", Duration::from_millis(40));
        controller.finish(&"finished", Duration::from_millis(40));

        assert_eq!(controller.prune_terminal_at(Duration::from_millis(50)), 2);
        assert_eq!(controller.tracks().len(), 1);
        assert_eq!(controller.tracks()[0].0, "active");
        assert!(
            controller
                .frame_demand_at(Duration::from_millis(50))
                .needs_frame()
        );
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

    #[test]
    fn reduced_motion_execution_publishes_final_value_without_frame_demand() {
        let execution = MotionScalarExecution::start_resolved(
            MotionPolicyInput::new(
                MotionPolicyContext::CommittedLayout,
                MotionModel::timeline(MotionSpec::committed_layout(MotionPreference::Reduced)),
            )
            .with_reduced_motion_final_state(true),
            0.0,
            1.0,
            0.0,
            Duration::ZERO,
        );

        let sample = execution.sample_at(Duration::ZERO);

        assert_eq!(sample.scalar_sample().state(), MotionRunState::Immediate);
        assert_eq!(sample.value(), 1.0);
        assert!(sample.complete());
        assert!(!sample.frame_demand().needs_frame());
    }

    #[test]
    fn progress_execution_samples_normalized_lifecycle() {
        let started_at = Instant::now();
        let progress = MotionProgressExecution::start_resolved(
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
            started_at,
        );

        let midpoint = progress.sample_since(started_at + Duration::from_millis(50));
        assert_eq!(midpoint.progress(), 0.5);
        assert!(midpoint.frame_demand().needs_frame());

        let complete = progress.sample_at(Duration::from_millis(120));
        assert_eq!(complete.progress(), 1.0);
        assert!(complete.complete());
        assert!(!complete.frame_demand().needs_frame());
    }
}
