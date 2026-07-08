//! Adapter-owned frame request helpers.

use crate::{MotionClockSample, MotionFrameDemand};
use std::time::Duration;

/// Renderer-neutral host state for one adapter-owned motion frame source.
///
/// The host does not schedule frames by itself. It records the latest motion demand and returns a
/// small decision object that the owning adapter can translate into its own frame request API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameHost {
    last_elapsed: Duration,
    last_frame_demand: MotionFrameDemand,
    last_reset_reason: Option<MotionFrameHostResetReason>,
    requested_frames: u64,
}

/// Reason an adapter starts a new local motion frame epoch.
///
/// Resetting an epoch clears stale elapsed time, frame demand, and requested-frame diagnostics.
/// Adapters should reset when they replace the motion identity or target, cancel an active run,
/// force a run to its final state, or prune terminal state after observing idle demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFrameHostResetReason {
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

impl MotionFrameHost {
    /// Creates an idle frame host.
    pub const fn new() -> Self {
        Self {
            last_elapsed: Duration::ZERO,
            last_frame_demand: MotionFrameDemand::Idle,
            last_reset_reason: None,
            requested_frames: 0,
        }
    }

    /// Returns the last clamped elapsed time observed by this host.
    pub const fn last_elapsed(&self) -> Duration {
        self.last_elapsed
    }

    /// Returns the latest frame demand observed by this host.
    pub const fn last_frame_demand(&self) -> MotionFrameDemand {
        self.last_frame_demand
    }

    /// Returns the reason the current adapter epoch was last reset.
    pub const fn last_reset_reason(&self) -> Option<MotionFrameHostResetReason> {
        self.last_reset_reason
    }

    /// Returns how many frame requests this host has asked the adapter to issue.
    pub const fn requested_frames(&self) -> u64 {
        self.requested_frames
    }

    /// Resets elapsed time and demand state for a new adapter-owned motion epoch.
    pub const fn reset(&mut self, reason: MotionFrameHostResetReason) {
        self.last_elapsed = Duration::ZERO;
        self.last_frame_demand = MotionFrameDemand::Idle;
        self.last_reset_reason = Some(reason);
        self.requested_frames = 0;
    }

    /// Observes a frame demand and returns the adapter decision for this render pass.
    pub fn observe(&mut self, frame_demand: MotionFrameDemand) -> MotionFrameHostUpdate {
        self.last_frame_demand = frame_demand;
        if frame_demand.needs_frame() {
            self.requested_frames = self.requested_frames.saturating_add(1);
        }
        MotionFrameHostUpdate {
            frame_demand,
            requested_frames: self.requested_frames,
        }
    }

    /// Combines many frame demands and returns the adapter decision for this render pass.
    pub fn observe_all(
        &mut self,
        demands: impl IntoIterator<Item = MotionFrameDemand>,
    ) -> MotionFrameHostUpdate {
        self.observe(MotionFrameDemand::combine_all(demands))
    }

    /// Samples motion from explicit adapter elapsed time and records the returned frame demand.
    pub fn sample_elapsed<T>(
        &mut self,
        requested_elapsed: Duration,
        sample: impl FnOnce(MotionClockSample) -> (T, MotionFrameDemand),
    ) -> MotionFrameHostSample<T> {
        let clock = MotionClockSample::from_elapsed(self.last_elapsed, requested_elapsed);
        self.last_elapsed = clock.elapsed();
        let (value, frame_demand) = sample(clock);
        let update = self.observe(frame_demand);
        MotionFrameHostSample {
            value,
            clock,
            update,
        }
    }
}

impl Default for MotionFrameHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter decision produced after a frame host observes motion demand.
#[must_use = "adapter frame updates must be translated into the owner's frame request API"]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameHostUpdate {
    frame_demand: MotionFrameDemand,
    requested_frames: u64,
}

impl MotionFrameHostUpdate {
    /// Returns the frame demand that produced this update.
    pub const fn frame_demand(self) -> MotionFrameDemand {
        self.frame_demand
    }

    /// Returns whether the adapter should request another frame.
    pub const fn should_request_frame(self) -> bool {
        self.frame_demand.needs_frame()
    }

    /// Returns the host's cumulative requested-frame count after this update.
    pub const fn requested_frames(self) -> u64 {
        self.requested_frames
    }
}

/// Value sampled through a frame host plus the host's adapter decision.
#[must_use = "frame host samples include the adapter's next-frame decision"]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionFrameHostSample<T> {
    value: T,
    clock: MotionClockSample,
    update: MotionFrameHostUpdate,
}

impl<T> MotionFrameHostSample<T> {
    /// Returns the sampled value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Consumes the sample and returns the sampled value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Returns the clamped clock used for sampling.
    pub const fn clock(&self) -> MotionClockSample {
        self.clock
    }

    /// Returns the host update produced after sampling.
    pub const fn update(&self) -> MotionFrameHostUpdate {
        self.update
    }

    /// Returns the frame demand that produced this sample.
    pub const fn frame_demand(&self) -> MotionFrameDemand {
        self.update.frame_demand()
    }

    /// Returns whether the adapter should request another frame.
    pub const fn should_request_frame(&self) -> bool {
        self.update.should_request_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotionFrameReason;

    #[test]
    fn idle_demand_does_not_request_frame() {
        let mut host = MotionFrameHost::new();

        let update = host.observe(MotionFrameDemand::Idle);

        assert!(!update.should_request_frame());
        assert_eq!(update.frame_demand(), MotionFrameDemand::Idle);
        assert_eq!(update.requested_frames(), 0);
        assert_eq!(host.last_frame_demand(), MotionFrameDemand::Idle);
    }

    #[test]
    fn active_demand_requests_frame_and_records_count() {
        let mut host = MotionFrameHost::new();
        let demand = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);

        let first = host.observe(demand);
        let second = host.observe(demand);

        assert!(first.should_request_frame());
        assert_eq!(first.requested_frames(), 1);
        assert!(second.should_request_frame());
        assert_eq!(second.requested_frames(), 2);
        assert_eq!(host.last_frame_demand(), demand);
    }

    #[test]
    fn combines_many_demands_into_one_adapter_decision() {
        let mut host = MotionFrameHost::new();
        let demand = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);

        let update = host.observe_all([MotionFrameDemand::Idle, demand, MotionFrameDemand::Idle]);

        assert!(update.should_request_frame());
        assert_eq!(update.frame_demand(), demand);
        assert_eq!(update.requested_frames(), 1);
    }

    #[test]
    fn sampling_clamps_non_monotonic_elapsed_time() {
        let mut host = MotionFrameHost::new();
        let demand = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);

        let first =
            host.sample_elapsed(Duration::from_millis(40), |clock| (clock.elapsed(), demand));
        let second =
            host.sample_elapsed(Duration::from_millis(10), |clock| (clock.elapsed(), demand));

        assert_eq!(*first.value(), Duration::from_millis(40));
        assert_eq!(*second.value(), Duration::from_millis(40));
        assert!(second.clock().clamped());
        assert_eq!(second.clock().delta(), Duration::ZERO);
        assert_eq!(host.last_elapsed(), Duration::from_millis(40));
    }
}
