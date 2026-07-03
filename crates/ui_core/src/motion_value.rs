//! Renderer-neutral scalar motion value state.

use std::time::Duration;

const DEFAULT_VELOCITY_STALE_AFTER: Duration = Duration::from_millis(50);

/// Stable owner token for one active value run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionValueRunOwner(u64);

impl MotionValueRunOwner {
    /// Creates a run owner from a stable numeric id.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the stable owner id.
    pub const fn id(self) -> u64 {
        self.0
    }
}

/// Result of assigning a new active run owner to a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionValueRunReplacement {
    /// No previous run owner existed.
    Started,
    /// A previous run owner was replaced and should be treated as cancelled.
    Replaced(MotionValueRunOwner),
}

/// Renderer-neutral scalar value with deterministic previous-frame velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionValue {
    current: f32,
    previous: f32,
    previous_frame: f32,
    previous_frame_at: Duration,
    updated_at: Duration,
    active_owner: Option<MotionValueRunOwner>,
    velocity_stale_after: Duration,
}

impl MotionValue {
    /// Creates a scalar value at the provided controller time.
    pub fn new(value: f32, now: Duration) -> Self {
        let value = sanitize(value, 0.0);
        Self {
            current: value,
            previous: value,
            previous_frame: value,
            previous_frame_at: now,
            updated_at: now,
            active_owner: None,
            velocity_stale_after: DEFAULT_VELOCITY_STALE_AFTER,
        }
    }

    /// Returns the current scalar value.
    pub const fn current(self) -> f32 {
        self.current
    }

    /// Returns the value before the latest set or jump.
    pub const fn previous(self) -> f32 {
        self.previous
    }

    /// Returns the value captured at the last explicit frame boundary.
    pub const fn previous_frame(self) -> f32 {
        self.previous_frame
    }

    /// Returns the controller time of the latest value update.
    pub const fn updated_at(self) -> Duration {
        self.updated_at
    }

    /// Returns the active run owner, if one exists.
    pub const fn active_owner(self) -> Option<MotionValueRunOwner> {
        self.active_owner
    }

    /// Returns a copy with a custom stale window for previous-frame velocity.
    pub const fn with_velocity_stale_after(mut self, stale_after: Duration) -> Self {
        self.velocity_stale_after = stale_after;
        self
    }

    /// Captures the current value as the previous-frame value.
    pub fn begin_frame(&mut self, now: Duration) {
        self.previous_frame = self.current;
        self.previous_frame_at = now;
    }

    /// Sets the current value while preserving active ownership.
    pub fn set(&mut self, value: f32, now: Duration) {
        let value = sanitize(value, self.current);
        self.previous = self.current;
        self.current = value;
        self.updated_at = now;
    }

    /// Jumps to a value and clears active run ownership.
    pub fn jump(&mut self, value: f32, now: Duration) {
        let value = sanitize(value, self.current);
        self.current = value;
        self.previous = value;
        self.previous_frame = value;
        self.previous_frame_at = now;
        self.updated_at = now;
        self.active_owner = None;
    }

    /// Starts or replaces the active run owner.
    pub fn start_run(&mut self, owner: MotionValueRunOwner) -> MotionValueRunReplacement {
        match self.active_owner.replace(owner) {
            Some(previous) if previous != owner => MotionValueRunReplacement::Replaced(previous),
            _ => MotionValueRunReplacement::Started,
        }
    }

    /// Cancels the active run owner when it matches the provided owner.
    pub fn cancel_run(&mut self, owner: MotionValueRunOwner) -> bool {
        if self.active_owner == Some(owner) {
            self.active_owner = None;
            true
        } else {
            false
        }
    }

    /// Cancels and returns the current active run owner.
    pub fn cancel_active_run(&mut self) -> Option<MotionValueRunOwner> {
        self.active_owner.take()
    }

    /// Returns deterministic previous-frame velocity at the provided controller time.
    pub fn velocity_at(self, now: Duration) -> f32 {
        let elapsed = now.saturating_sub(self.previous_frame_at);
        if elapsed.is_zero() || elapsed > self.velocity_stale_after {
            return 0.0;
        }
        (self.current - self.previous_frame) / elapsed.as_secs_f32()
    }
}

fn sanitize(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_scalar_value_tracks_current_previous_and_velocity() {
        let mut value = MotionValue::new(0.0, Duration::ZERO);
        value.begin_frame(Duration::ZERO);

        value.set(10.0, Duration::from_millis(16));

        assert_eq!(value.current(), 10.0);
        assert_eq!(value.previous(), 0.0);
        assert_eq!(value.previous_frame(), 0.0);
        assert!((value.velocity_at(Duration::from_millis(16)) - 625.0).abs() < 0.01);
    }

    #[test]
    fn stale_previous_frame_velocity_reports_zero() {
        let mut value = MotionValue::new(0.0, Duration::ZERO)
            .with_velocity_stale_after(Duration::from_millis(20));
        value.begin_frame(Duration::ZERO);
        value.set(10.0, Duration::from_millis(16));

        assert_eq!(value.velocity_at(Duration::from_millis(40)), 0.0);
    }

    #[test]
    fn jump_resets_previous_values_and_clears_active_owner() {
        let mut value = MotionValue::new(0.0, Duration::ZERO);
        assert_eq!(
            value.start_run(MotionValueRunOwner::new(1)),
            MotionValueRunReplacement::Started
        );

        value.jump(5.0, Duration::from_millis(10));

        assert_eq!(value.current(), 5.0);
        assert_eq!(value.previous(), 5.0);
        assert_eq!(value.previous_frame(), 5.0);
        assert_eq!(value.active_owner(), None);
        assert_eq!(value.velocity_at(Duration::from_millis(10)), 0.0);
    }

    #[test]
    fn replacing_active_owner_reports_cancelled_owner() {
        let mut value = MotionValue::new(0.0, Duration::ZERO);
        let first = MotionValueRunOwner::new(1);
        let second = MotionValueRunOwner::new(2);

        assert_eq!(value.start_run(first), MotionValueRunReplacement::Started);
        assert_eq!(
            value.start_run(second),
            MotionValueRunReplacement::Replaced(first)
        );
        assert!(!value.cancel_run(first));
        assert!(value.cancel_run(second));
        assert_eq!(value.active_owner(), None);
    }
}
