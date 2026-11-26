//! Renderer-neutral runtime helpers for deterministic UI motion.

use std::time::Duration;

use crate::motion::MotionSpec;

/// Runtime state for sampled motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionRunState {
    /// The motion spec completed immediately.
    Immediate,
    /// The motion run is still active.
    Active,
    /// The motion run reached its final state.
    Completed,
    /// The motion run was cancelled before reaching its final state.
    Cancelled,
}

impl MotionRunState {
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
pub(crate) struct MotionTimelineSample {
    state: MotionRunState,
    elapsed: Duration,
    raw_progress: f32,
    progress: f32,
}

impl MotionTimelineSample {
    /// Creates a sample from explicit values.
    pub(crate) const fn new(
        state: MotionRunState,
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
    pub(crate) const fn state(self) -> MotionRunState {
        self.state
    }

    /// Returns the elapsed time used for this sample.
    pub(crate) const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns the unclamped easing input after duration normalization.
    #[cfg(test)]
    pub(crate) const fn raw_progress(self) -> f32 {
        self.raw_progress
    }

    /// Returns the eased progress.
    pub(crate) const fn progress(self) -> f32 {
        self.progress
    }

    /// Returns whether the semantic final state has been reached.
    pub(crate) const fn reached_final_state(self) -> bool {
        self.state.reached_final_state()
    }
}

/// A deterministic timeline for one UI motion transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MotionTimeline;

impl MotionTimeline {
    /// Samples a motion spec using an explicit elapsed duration.
    pub(crate) fn sample_elapsed(spec: MotionSpec, elapsed: Duration) -> MotionTimelineSample {
        if spec.is_immediate() {
            return MotionTimelineSample::new(MotionRunState::Immediate, elapsed, 1.0, 1.0);
        }

        let duration = spec.duration().as_duration();
        if duration.is_zero() {
            return MotionTimelineSample::new(MotionRunState::Immediate, elapsed, 1.0, 1.0);
        }

        let raw_progress = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = spec.easing().sample(raw_progress);
        let state = if raw_progress >= 1.0 {
            MotionRunState::Completed
        } else {
            MotionRunState::Active
        };
        MotionTimelineSample::new(state, elapsed, raw_progress, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{MotionDuration, MotionEasing, MotionPreference};

    #[test]
    fn timeline_samples_start_midpoint_and_completion() {
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );

        let start = MotionTimeline::sample_elapsed(spec, Duration::ZERO);
        assert_eq!(start.state(), MotionRunState::Active);
        assert_eq!(start.elapsed(), Duration::from_millis(0));
        assert_eq!(start.raw_progress(), 0.0);
        assert_eq!(start.progress(), 0.0);

        let midpoint = MotionTimeline::sample_elapsed(spec, Duration::from_millis(100));
        assert_eq!(midpoint.state(), MotionRunState::Active);
        assert_eq!(midpoint.raw_progress(), 0.5);
        assert_eq!(midpoint.progress(), 0.5);

        let complete = MotionTimeline::sample_elapsed(spec, Duration::from_millis(250));
        assert_eq!(complete.state(), MotionRunState::Completed);
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

        assert_eq!(sample.state(), MotionRunState::Immediate);
        assert_eq!(sample.raw_progress(), 1.0);
        assert_eq!(sample.progress(), 1.0);
        assert!(sample.reached_final_state());
    }
}
