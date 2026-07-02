//! Renderer-neutral runtime helpers for deterministic UI motion.

use crate::MotionSpec;
use std::{
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

/// Runtime state for a sampled motion timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionTimelineState {
    /// The motion spec completed immediately.
    Immediate,
    /// The motion timeline is still active.
    Active,
    /// The motion timeline reached its final state.
    Completed,
    /// The motion timeline was cancelled before reaching its final state.
    Cancelled,
}

impl MotionTimelineState {
    /// Returns whether callers should continue requesting animation frames.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns whether the timeline no longer needs animation frames.
    pub const fn is_terminal(self) -> bool {
        !self.is_active()
    }

    /// Returns whether the semantic final state has been reached.
    pub const fn reached_final_state(self) -> bool {
        matches!(self, Self::Immediate | Self::Completed)
    }
}

/// A sampled point on a motion timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionTimelineSample {
    state: MotionTimelineState,
    elapsed: Duration,
    raw_progress: f32,
    progress: f32,
}

impl MotionTimelineSample {
    /// Creates a sample from explicit values.
    pub const fn new(
        state: MotionTimelineState,
        elapsed: Duration,
        raw_progress: f32,
        progress: f32,
    ) -> Self {
        Self {
            state,
            elapsed,
            raw_progress,
            progress,
        }
    }

    /// Returns the sampled timeline state.
    pub const fn state(self) -> MotionTimelineState {
        self.state
    }

    /// Returns the elapsed time used for this sample.
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns the unclamped easing input after duration normalization.
    pub const fn raw_progress(self) -> f32 {
        self.raw_progress
    }

    /// Returns the eased progress.
    pub const fn progress(self) -> f32 {
        self.progress
    }

    /// Returns whether the timeline should continue requesting frames.
    pub const fn is_active(self) -> bool {
        self.state.is_active()
    }

    /// Returns whether the semantic final state has been reached.
    pub const fn reached_final_state(self) -> bool {
        self.state.reached_final_state()
    }
}

/// A deterministic timeline for one UI motion transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionTimeline {
    spec: MotionSpec,
    started_at: Instant,
    cancelled_at: Option<Instant>,
}

impl MotionTimeline {
    /// Creates a timeline that starts at the provided instant.
    pub const fn new(spec: MotionSpec, started_at: Instant) -> Self {
        Self {
            spec,
            started_at,
            cancelled_at: None,
        }
    }

    /// Returns the motion specification used by this timeline.
    pub const fn spec(self) -> MotionSpec {
        self.spec
    }

    /// Returns the instant at which the timeline started.
    pub const fn started_at(self) -> Instant {
        self.started_at
    }

    /// Returns the instant at which the timeline was cancelled.
    pub const fn cancelled_at(self) -> Option<Instant> {
        self.cancelled_at
    }

    /// Marks the timeline as cancelled at the provided instant.
    pub fn cancel_at(&mut self, cancelled_at: Instant) {
        self.cancelled_at = Some(cancelled_at);
    }

    /// Samples the timeline at the provided instant.
    pub fn sample(self, now: Instant) -> MotionTimelineSample {
        let effective_now = self.cancelled_at.unwrap_or(now);
        let elapsed = effective_now.saturating_duration_since(self.started_at);
        let mut sample = Self::sample_elapsed(self.spec, elapsed);
        if self.cancelled_at.is_some() && !sample.reached_final_state() {
            sample.state = MotionTimelineState::Cancelled;
        }
        sample
    }

    /// Samples a motion spec using an explicit elapsed duration.
    pub fn sample_elapsed(spec: MotionSpec, elapsed: Duration) -> MotionTimelineSample {
        if spec.is_immediate() {
            return MotionTimelineSample::new(MotionTimelineState::Immediate, elapsed, 1.0, 1.0);
        }

        let duration = spec.duration().as_duration();
        if duration.is_zero() {
            return MotionTimelineSample::new(MotionTimelineState::Immediate, elapsed, 1.0, 1.0);
        }

        let raw_progress = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = spec.easing().sample(raw_progress);
        let state = if raw_progress >= 1.0 {
            MotionTimelineState::Completed
        } else {
            MotionTimelineState::Active
        };
        MotionTimelineSample::new(state, elapsed, raw_progress, progress)
    }
}

/// A stable-id value captured from a motion sample or target state.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSnapshot<K, V> {
    id: K,
    value: V,
}

impl<K, V> MotionSnapshot<K, V> {
    /// Creates a stable-id snapshot.
    pub const fn new(id: K, value: V) -> Self {
        Self { id, value }
    }

    /// Returns the stable identity.
    pub const fn id(&self) -> &K {
        &self.id
    }

    /// Returns the captured value.
    pub const fn value(&self) -> &V {
        &self.value
    }

    /// Consumes the snapshot and returns its parts.
    pub fn into_parts(self) -> (K, V) {
        (self.id, self.value)
    }
}

/// A target item paired with the currently sampled value for the same identity.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionRetargetItem<K, S, T = S> {
    id: K,
    sampled: Option<S>,
    target: T,
}

impl<K, S, T> MotionRetargetItem<K, S, T> {
    /// Creates a retargeted item.
    pub const fn new(id: K, sampled: Option<S>, target: T) -> Self {
        Self {
            id,
            sampled,
            target,
        }
    }

    /// Returns the stable identity.
    pub const fn id(&self) -> &K {
        &self.id
    }

    /// Returns the sampled value when the identity existed in the interrupted transition.
    pub const fn sampled(&self) -> Option<&S> {
        self.sampled.as_ref()
    }

    /// Returns the target value.
    pub const fn target(&self) -> &T {
        &self.target
    }

    /// Consumes the item and returns its parts.
    pub fn into_parts(self) -> (K, Option<S>, T) {
        (self.id, self.sampled, self.target)
    }
}

/// Stable-id retargeting result for a new target set.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionRetargetSet<K, S, T = S> {
    targets: Vec<MotionRetargetItem<K, S, T>>,
    leaving: Vec<MotionSnapshot<K, S>>,
}

impl<K, S, T> MotionRetargetSet<K, S, T> {
    /// Creates a retarget set.
    pub fn new(
        targets: Vec<MotionRetargetItem<K, S, T>>,
        leaving: Vec<MotionSnapshot<K, S>>,
    ) -> Self {
        Self { targets, leaving }
    }

    /// Returns the target items in target order.
    pub fn targets(&self) -> &[MotionRetargetItem<K, S, T>] {
        &self.targets
    }

    /// Returns sampled items that were not present in the target set.
    pub fn leaving(&self) -> &[MotionSnapshot<K, S>] {
        &self.leaving
    }

    /// Consumes the set and returns its parts.
    pub fn into_parts(self) -> (Vec<MotionRetargetItem<K, S, T>>, Vec<MotionSnapshot<K, S>>) {
        (self.targets, self.leaving)
    }
}

/// Matches sampled values to a new target set by stable identity.
///
/// The returned targets preserve target order. The returned leaving snapshots preserve sampled
/// order for identities that are absent from the target set.
pub fn retarget_motion_snapshots<K, S, T>(
    sampled: impl IntoIterator<Item = MotionSnapshot<K, S>>,
    targets: impl IntoIterator<Item = MotionSnapshot<K, T>>,
) -> MotionRetargetSet<K, S, T>
where
    K: Clone + Eq + Hash,
{
    let mut sampled = sampled.into_iter().map(Some).collect::<Vec<_>>();
    let mut sampled_indices = HashMap::new();
    for (index, snapshot) in sampled.iter().enumerate() {
        if let Some(snapshot) = snapshot {
            sampled_indices.entry(snapshot.id.clone()).or_insert(index);
        }
    }

    let targets = targets
        .into_iter()
        .map(|target| {
            let (id, target_value) = target.into_parts();
            let sampled_value = sampled_indices
                .get(&id)
                .and_then(|index| sampled[*index].take())
                .map(|snapshot| snapshot.value);
            MotionRetargetItem::new(id, sampled_value, target_value)
        })
        .collect::<Vec<_>>();
    let leaving = sampled.into_iter().flatten().collect::<Vec<_>>();

    MotionRetargetSet::new(targets, leaving)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MotionDuration, MotionEasing, MotionPreference};

    #[test]
    fn timeline_samples_start_midpoint_and_completion() {
        let started_at = Instant::now();
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );
        let timeline = MotionTimeline::new(spec, started_at);

        let start = timeline.sample(started_at);
        assert_eq!(start.state(), MotionTimelineState::Active);
        assert_eq!(start.elapsed(), Duration::from_millis(0));
        assert_eq!(start.raw_progress(), 0.0);
        assert_eq!(start.progress(), 0.0);

        let midpoint = timeline.sample(started_at + Duration::from_millis(100));
        assert_eq!(midpoint.state(), MotionTimelineState::Active);
        assert_eq!(midpoint.raw_progress(), 0.5);
        assert_eq!(midpoint.progress(), 0.5);

        let complete = timeline.sample(started_at + Duration::from_millis(250));
        assert_eq!(complete.state(), MotionTimelineState::Completed);
        assert_eq!(complete.raw_progress(), 1.0);
        assert_eq!(complete.progress(), 1.0);
        assert!(complete.reached_final_state());
    }

    #[test]
    fn reduced_motion_samples_as_immediate() {
        let sample = MotionTimeline::sample_elapsed(
            MotionSpec::layout(MotionPreference::Reduced),
            Duration::from_millis(0),
        );

        assert_eq!(sample.state(), MotionTimelineState::Immediate);
        assert_eq!(sample.raw_progress(), 1.0);
        assert_eq!(sample.progress(), 1.0);
        assert!(sample.reached_final_state());
    }

    #[test]
    fn cancelled_timeline_reports_cancelled_sampled_progress() {
        let started_at = Instant::now();
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );
        let mut timeline = MotionTimeline::new(spec, started_at);

        timeline.cancel_at(started_at + Duration::from_millis(80));
        let sample = timeline.sample(started_at + Duration::from_millis(160));

        assert_eq!(sample.state(), MotionTimelineState::Cancelled);
        assert!((sample.raw_progress() - 0.4).abs() < f32::EPSILON);
        assert!((sample.progress() - 0.4).abs() < f32::EPSILON);
        assert!(!sample.reached_final_state());
    }

    #[test]
    fn retarget_snapshots_match_by_identity_and_report_missing_items() {
        let retarget = retarget_motion_snapshots(
            [
                MotionSnapshot::new("left", 0.25),
                MotionSnapshot::new("center", 0.5),
                MotionSnapshot::new("right", 0.25),
            ],
            [
                MotionSnapshot::new("center", 0.7),
                MotionSnapshot::new("inspector", 0.3),
            ],
        );

        assert_eq!(retarget.targets().len(), 2);
        assert_eq!(retarget.targets()[0].id(), &"center");
        assert_eq!(retarget.targets()[0].sampled(), Some(&0.5));
        assert_eq!(retarget.targets()[0].target(), &0.7);
        assert_eq!(retarget.targets()[1].id(), &"inspector");
        assert_eq!(retarget.targets()[1].sampled(), None);
        assert_eq!(retarget.targets()[1].target(), &0.3);

        let leaving_ids = retarget
            .leaving()
            .iter()
            .map(MotionSnapshot::id)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(leaving_ids, ["left", "right"]);
    }
}
