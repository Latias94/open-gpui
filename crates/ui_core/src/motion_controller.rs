//! Renderer-neutral motion controller contracts.

use crate::motion_spring::{MotionModel, MotionSpringSample};
use crate::{MotionSpec, MotionTimelineState};
use std::time::Duration;

/// Renderer-neutral frame demand returned by motion controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFrameDemand {
    /// No more animation frames are required.
    Idle,
    /// At least one track is active and the adapter should request another frame.
    NeedsFrame,
}

impl MotionFrameDemand {
    /// Returns whether another frame should be requested by the adapter.
    pub const fn needs_frame(self) -> bool {
        matches!(self, Self::NeedsFrame)
    }

    fn from_active(active: bool) -> Self {
        if active { Self::NeedsFrame } else { Self::Idle }
    }

    fn combine(self, other: Self) -> Self {
        if self.needs_frame() || other.needs_frame() {
            Self::NeedsFrame
        } else {
            Self::Idle
        }
    }
}

/// One scalar motion track sampled by deterministic elapsed time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionScalarTrack {
    model: MotionModel,
    from: f32,
    target: f32,
    initial_velocity: f32,
    started_at: Duration,
    cancelled_at: Option<Duration>,
}

impl MotionScalarTrack {
    /// Starts a scalar track at the provided controller time.
    pub const fn start(
        model: MotionModel,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Duration,
    ) -> Self {
        Self {
            model,
            from,
            target,
            initial_velocity,
            started_at,
            cancelled_at: None,
        }
    }

    /// Creates an immediate scalar track at a fixed value.
    pub const fn immediate(value: f32, started_at: Duration) -> Self {
        Self::start(
            MotionModel::timeline(MotionSpec::immediate()),
            value,
            value,
            0.0,
            started_at,
        )
    }

    /// Returns the motion model.
    pub const fn model(self) -> MotionModel {
        self.model
    }

    /// Returns the source value.
    pub const fn from(self) -> f32 {
        self.from
    }

    /// Returns the target value.
    pub const fn target(self) -> f32 {
        self.target
    }

    /// Returns the initial velocity.
    pub const fn initial_velocity(self) -> f32 {
        self.initial_velocity
    }

    /// Returns the controller time at which the track started.
    pub const fn started_at(self) -> Duration {
        self.started_at
    }

    /// Returns the controller time at which the track was cancelled.
    pub const fn cancelled_at(self) -> Option<Duration> {
        self.cancelled_at
    }

    /// Cancels the track at the provided controller time.
    pub fn cancel_at(&mut self, cancelled_at: Duration) {
        self.cancelled_at = Some(cancelled_at);
    }

    /// Retargets the track from its sampled value and velocity.
    pub fn retarget(self, model: MotionModel, target: f32, now: Duration) -> Self {
        let sample = self.sample_at(now);
        Self::start(model, sample.value(), target, sample.velocity(), now)
    }

    /// Samples the track at the provided controller time.
    pub fn sample_at(self, now: Duration) -> MotionSpringSample {
        let effective_now = self.cancelled_at.unwrap_or(now);
        let elapsed = effective_now.saturating_sub(self.started_at);
        let mut sample = self.model.sample_scalar_elapsed(
            self.from,
            self.target,
            self.initial_velocity,
            elapsed,
        );
        if self.cancelled_at.is_some() && sample.state().is_active() {
            sample = MotionSpringSample::new(
                MotionTimelineState::Cancelled,
                sample.elapsed(),
                sample.value(),
                sample.velocity(),
                sample.target(),
            );
        }
        sample
    }

    /// Returns whether this track needs another adapter-owned frame.
    pub fn frame_demand_at(self, now: Duration) -> MotionFrameDemand {
        MotionFrameDemand::from_active(self.sample_at(now).is_active())
    }
}

/// A sampled keyed scalar motion track.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionScalarTrackSample<K> {
    key: K,
    sample: MotionSpringSample,
}

impl<K> MotionScalarTrackSample<K> {
    /// Creates a keyed track sample.
    pub const fn new(key: K, sample: MotionSpringSample) -> Self {
        Self { key, sample }
    }

    /// Returns the track key.
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the scalar motion sample.
    pub const fn sample(&self) -> MotionSpringSample {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionDuration, MotionEasing, MotionModel, MotionPreference, MotionSpec, MotionSpringSpec,
        MotionTimelineState,
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

        assert_eq!(sample.state(), MotionTimelineState::Cancelled);
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
        assert_eq!(sample.state(), MotionTimelineState::Immediate);
        assert_eq!(sample.value(), 0.75);
        assert!(
            !track
                .frame_demand_at(Duration::from_millis(10))
                .needs_frame()
        );
    }
}
