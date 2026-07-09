//! Renderer-neutral sequence plans for composing many motion tracks.

use crate::{
    MotionFrameDemand, MotionFrameReason, MotionRunState, MotionScalarSample,
    transition::MotionTransition,
};
use std::time::Duration;

/// A deterministic sequence of scalar motion steps.
///
/// The sequence owns relative timing and frame-demand aggregation, but it does not schedule frames,
/// mutate render state, or know about GPUI elements. Adapters sample it with elapsed time and map
/// the returned values into their own presentation state.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSequence<K> {
    steps: Vec<MotionSequenceStep<K>>,
}

impl<K> Default for MotionSequence<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> MotionSequence<K> {
    /// Creates an empty sequence.
    pub const fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Returns the steps in insertion order.
    pub fn steps(&self) -> &[MotionSequenceStep<K>] {
        &self.steps
    }

    /// Returns whether the sequence has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Returns the number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Inserts a step at an absolute sequence elapsed time.
    pub fn insert_at(
        &mut self,
        key: K,
        transition: MotionTransition,
        start_at: Duration,
    ) -> &mut Self {
        self.steps
            .push(MotionSequenceStep::new(key, transition, start_at));
        self
    }

    /// Appends a step after the current sequence duration hint.
    pub fn append(&mut self, key: K, transition: MotionTransition) -> &mut Self {
        self.insert_at(key, transition, self.duration_hint())
    }

    /// Inserts a step at the previous step's start time.
    pub fn insert_with_previous(&mut self, key: K, transition: MotionTransition) -> &mut Self {
        let start_at = self
            .steps
            .last()
            .map(MotionSequenceStep::start_at)
            .unwrap_or(Duration::ZERO);
        self.insert_at(key, transition, start_at)
    }

    /// Inserts a step after the previous step's end hint plus delay.
    pub fn insert_after_previous(
        &mut self,
        key: K,
        transition: MotionTransition,
        delay: Duration,
    ) -> &mut Self {
        let start_at = self
            .steps
            .last()
            .map(MotionSequenceStep::end_hint)
            .unwrap_or(Duration::ZERO);
        self.insert_at(key, transition, saturating_duration_add(start_at, delay))
    }

    /// Inserts many steps with a fixed stagger from a start time.
    pub fn insert_staggered(
        &mut self,
        keys: impl IntoIterator<Item = K>,
        transition: MotionTransition,
        start_at: Duration,
        stagger: Duration,
    ) -> &mut Self {
        let mut next_start = start_at;
        for key in keys {
            self.insert_at(key, transition, next_start);
            next_start = saturating_duration_add(next_start, stagger);
        }
        self
    }

    /// Returns the sequence duration hint from the latest step end hint.
    pub fn duration_hint(&self) -> Duration {
        self.steps
            .iter()
            .map(MotionSequenceStep::end_hint)
            .max()
            .unwrap_or(Duration::ZERO)
    }
}

impl<K: Clone> MotionSequence<K> {
    /// Samples all steps at sequence elapsed time.
    pub fn sample_at(&self, elapsed: Duration) -> MotionSequenceSample<K> {
        let steps = self
            .steps
            .iter()
            .map(|step| step.sample_at(elapsed))
            .collect::<Vec<_>>();
        let needs_frame = steps
            .iter()
            .any(|step| step.state().needs_frame_for_sequence());
        let frame_demand = if needs_frame {
            MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender)
        } else {
            MotionFrameDemand::Idle
        };
        MotionSequenceSample::new(steps, frame_demand)
    }
}

/// One keyed sequence step.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSequenceStep<K> {
    key: K,
    transition: MotionTransition,
    start_at: Duration,
    duration_hint: Duration,
}

impl<K> MotionSequenceStep<K> {
    /// Creates a sequence step at an absolute sequence elapsed time.
    pub fn new(key: K, transition: MotionTransition, start_at: Duration) -> Self {
        Self {
            key,
            transition,
            start_at,
            duration_hint: transition.resolve_plan().model().sequence_duration_hint(),
        }
    }

    /// Returns the stable step key.
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the step transition.
    pub const fn transition(&self) -> MotionTransition {
        self.transition
    }

    /// Returns the absolute sequence time at which this step starts.
    pub const fn start_at(&self) -> Duration {
        self.start_at
    }

    /// Returns the duration hint used for timeline composition.
    pub const fn duration_hint(&self) -> Duration {
        self.duration_hint
    }

    /// Returns the hinted end time for this step.
    pub fn end_hint(&self) -> Duration {
        saturating_duration_add(self.start_at, self.duration_hint)
    }
}

impl<K: Clone> MotionSequenceStep<K> {
    /// Samples this step at sequence elapsed time.
    pub fn sample_at(&self, elapsed: Duration) -> MotionSequenceStepSample<K> {
        if elapsed < self.start_at {
            return MotionSequenceStepSample::pending(self.key.clone());
        }

        let local_elapsed = elapsed.saturating_sub(self.start_at);
        let sample = self
            .transition
            .sample_scalar_elapsed(0.0, 1.0, 0.0, local_elapsed);
        MotionSequenceStepSample::from_scalar(self.key.clone(), sample)
    }
}

/// Sequence-level state for one sampled step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSequenceStepState {
    /// The step has not reached its start time yet.
    Pending,
    /// The step is active and should keep requesting frames.
    Active,
    /// The step completed immediately due to policy or reduced motion.
    Immediate,
    /// The step reached its final state.
    Completed,
    /// The step was cancelled before reaching its final state.
    Cancelled,
}

impl MotionSequenceStepState {
    /// Returns whether this state keeps the sequence frame demand active.
    pub const fn needs_frame_for_sequence(self) -> bool {
        matches!(self, Self::Pending | Self::Active)
    }

    /// Returns whether this state is terminal.
    pub const fn is_terminal(self) -> bool {
        !self.needs_frame_for_sequence()
    }

    /// Returns whether this state reached the final semantic state.
    pub const fn reached_final_state(self) -> bool {
        matches!(self, Self::Immediate | Self::Completed)
    }

    fn from_run_state(state: MotionRunState) -> Self {
        match state {
            MotionRunState::Immediate => Self::Immediate,
            MotionRunState::Active => Self::Active,
            MotionRunState::Completed => Self::Completed,
            MotionRunState::Cancelled => Self::Cancelled,
        }
    }
}

/// Sample for one keyed sequence step.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSequenceStepSample<K> {
    key: K,
    state: MotionSequenceStepState,
    elapsed: Duration,
    value: f32,
    velocity: f32,
    target: f32,
}

impl<K> MotionSequenceStepSample<K> {
    fn pending(key: K) -> Self {
        Self {
            key,
            state: MotionSequenceStepState::Pending,
            elapsed: Duration::ZERO,
            value: 0.0,
            velocity: 0.0,
            target: 1.0,
        }
    }

    fn from_scalar(key: K, sample: MotionScalarSample) -> Self {
        Self {
            key,
            state: MotionSequenceStepState::from_run_state(sample.state()),
            elapsed: sample.elapsed(),
            value: sample.value(),
            velocity: sample.velocity(),
            target: sample.target(),
        }
    }

    /// Returns the stable step key.
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the sequence-level step state.
    pub const fn state(&self) -> MotionSequenceStepState {
        self.state
    }

    /// Returns local elapsed time since the step started.
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns sampled scalar progress.
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Returns sampled scalar velocity.
    pub const fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Returns the scalar target.
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// Returns whether this step keeps requesting frames.
    pub const fn needs_frame(&self) -> bool {
        self.state.needs_frame_for_sequence()
    }
}

/// Sample for a full motion sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSequenceSample<K> {
    steps: Vec<MotionSequenceStepSample<K>>,
    frame_demand: MotionFrameDemand,
}

impl<K> MotionSequenceSample<K> {
    /// Creates a sequence sample.
    pub fn new(steps: Vec<MotionSequenceStepSample<K>>, frame_demand: MotionFrameDemand) -> Self {
        Self {
            steps,
            frame_demand,
        }
    }

    /// Returns sampled steps in sequence insertion order.
    pub fn steps(&self) -> &[MotionSequenceStepSample<K>] {
        &self.steps
    }

    /// Returns the aggregated frame demand for this sample.
    pub const fn frame_demand(&self) -> MotionFrameDemand {
        self.frame_demand
    }

    /// Returns whether every step is terminal.
    pub const fn complete(&self) -> bool {
        !self.frame_demand.needs_frame()
    }
}

impl<K: PartialEq> MotionSequenceSample<K> {
    /// Returns the sample for a key.
    pub fn step(&self, key: &K) -> Option<&MotionSequenceStepSample<K>> {
        self.steps.iter().find(|step| step.key() == key)
    }
}

fn saturating_duration_add(left: Duration, right: Duration) -> Duration {
    left.checked_add(right).unwrap_or(Duration::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionDuration, MotionEasing, MotionIntent, MotionPreference,
        spring::{MotionSpringPreset, MotionSpringSpec},
    };

    fn linear_transition(duration: Duration) -> MotionTransition {
        MotionTransition::duration(
            MotionIntent::CommittedLayout,
            MotionPreference::Animated,
            MotionDuration::Custom(duration),
            MotionEasing::Linear,
        )
    }

    #[test]
    fn sequence_positions_append_with_previous_and_after_previous() {
        let transition = linear_transition(Duration::from_millis(100));
        let mut sequence = MotionSequence::new();

        sequence
            .append("first", transition)
            .insert_with_previous("parallel", transition)
            .insert_after_previous("delayed", transition, Duration::from_millis(20));

        assert_eq!(sequence.steps()[0].start_at(), Duration::ZERO);
        assert_eq!(sequence.steps()[1].start_at(), Duration::ZERO);
        assert_eq!(sequence.steps()[2].start_at(), Duration::from_millis(120));
        assert_eq!(sequence.duration_hint(), Duration::from_millis(220));
    }

    #[test]
    fn staggered_steps_preserve_start_offsets() {
        let transition = linear_transition(Duration::from_millis(50));
        let mut sequence = MotionSequence::new();

        sequence.insert_staggered(
            ["a", "b", "c"],
            transition,
            Duration::from_millis(10),
            Duration::from_millis(20),
        );

        assert_eq!(sequence.steps()[0].start_at(), Duration::from_millis(10));
        assert_eq!(sequence.steps()[1].start_at(), Duration::from_millis(30));
        assert_eq!(sequence.steps()[2].start_at(), Duration::from_millis(50));
        assert_eq!(sequence.duration_hint(), Duration::from_millis(100));
    }

    #[test]
    fn sequence_samples_pending_active_and_completed_steps() {
        let transition = linear_transition(Duration::from_millis(100));
        let mut sequence = MotionSequence::new();
        sequence.insert_at("row", transition, Duration::from_millis(50));

        let pending = sequence.sample_at(Duration::ZERO);
        let pending_step = pending.step(&"row").expect("row step");
        assert_eq!(pending_step.state(), MotionSequenceStepState::Pending);
        assert!(pending.frame_demand().needs_frame());
        assert!(!pending.complete());

        let active = sequence.sample_at(Duration::from_millis(100));
        let active_step = active.step(&"row").expect("row step");
        assert_eq!(active_step.state(), MotionSequenceStepState::Active);
        assert_eq!(active_step.elapsed(), Duration::from_millis(50));
        assert_eq!(active_step.value(), 0.5);
        assert!(active.frame_demand().needs_frame());

        let complete = sequence.sample_at(Duration::from_millis(160));
        let complete_step = complete.step(&"row").expect("row step");
        assert_eq!(complete_step.state(), MotionSequenceStepState::Completed);
        assert_eq!(complete_step.value(), 1.0);
        assert!(!complete.frame_demand().needs_frame());
        assert!(complete.complete());
    }

    #[test]
    fn reduced_motion_step_completes_without_frame_demand_at_start() {
        let transition = MotionTransition::committed_layout(MotionPreference::Reduced);
        let mut sequence = MotionSequence::new();
        sequence.insert_at("panel", transition, Duration::ZERO);

        let sample = sequence.sample_at(Duration::ZERO);
        let step = sample.step(&"panel").expect("panel step");

        assert_eq!(step.state(), MotionSequenceStepState::Immediate);
        assert_eq!(step.value(), 1.0);
        assert!(step.state().reached_final_state());
        assert!(sample.complete());
    }

    #[test]
    fn spring_sequence_duration_hint_uses_review_duration() {
        let spring = MotionSpringSpec::layout(MotionPreference::Animated);
        let transition = MotionTransition::committed_layout(MotionPreference::Animated);
        let mut sequence = MotionSequence::new();

        sequence
            .append("spring", transition)
            .append("next", transition);

        assert_eq!(
            sequence.steps()[0].duration_hint(),
            spring.physics().review_duration()
        );
        assert_eq!(
            sequence.steps()[1].start_at(),
            spring.physics().review_duration()
        );
        assert_eq!(spring.preset(), Some(MotionSpringPreset::Layout));
    }
}
