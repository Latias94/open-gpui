//! Renderer-neutral spring sampling for layout-like UI motion.

use crate::{MotionPreference, MotionSpec, MotionTimelineState};
use std::time::{Duration, Instant};

const DEFAULT_MASS: f32 = 1.0;
const DEFAULT_STIFFNESS: f32 = 260.0;
const DEFAULT_DAMPING: f32 = 28.0;
const DEFAULT_REST_DELTA: f32 = 0.001;
const DEFAULT_REST_SPEED: f32 = 0.01;
const MIN_POSITIVE: f32 = 0.000_001;
const MAX_PHYSICS_VALUE: f32 = 1_000_000.0;

/// Reviewable spring presets for layout-like UI motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSpringPreset {
    /// Short, subtle affordance motion.
    Affordance,
    /// Committed layout motion such as collapse, expand, insert, or remove.
    Layout,
    /// Continuity motion for retargeted pane, divider, or zoom transitions.
    Continuity,
}

/// Renderer-neutral spring physics parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringPhysics {
    mass: f32,
    stiffness: f32,
    damping: f32,
    rest_delta: f32,
    rest_speed: f32,
    bounce: f32,
    review_duration: Duration,
}

impl MotionSpringPhysics {
    /// Maximum bounce value accepted by professional UI policy.
    pub const MAX_REVIEWABLE_BOUNCE: f32 = 0.35;

    /// Creates sanitized spring physics parameters.
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            mass: sanitize_positive(mass, DEFAULT_MASS),
            stiffness: sanitize_positive(stiffness, DEFAULT_STIFFNESS),
            damping: sanitize_non_negative(damping, DEFAULT_DAMPING),
            rest_delta: DEFAULT_REST_DELTA,
            rest_speed: DEFAULT_REST_SPEED,
            bounce: 0.0,
            review_duration: Duration::from_millis(180),
        }
    }

    /// Returns the mass parameter.
    pub const fn mass(self) -> f32 {
        self.mass
    }

    /// Returns the stiffness parameter.
    pub const fn stiffness(self) -> f32 {
        self.stiffness
    }

    /// Returns the damping parameter.
    pub const fn damping(self) -> f32 {
        self.damping
    }

    /// Returns the position rest threshold.
    pub const fn rest_delta(self) -> f32 {
        self.rest_delta
    }

    /// Returns the velocity rest threshold.
    pub const fn rest_speed(self) -> f32 {
        self.rest_speed
    }

    /// Returns the review-facing bounce value.
    pub const fn bounce(self) -> f32 {
        self.bounce
    }

    /// Returns the expected review duration for policy checks.
    pub const fn review_duration(self) -> Duration {
        self.review_duration
    }

    /// Returns a copy with a sanitized position rest threshold.
    pub fn with_rest_delta(mut self, rest_delta: f32) -> Self {
        self.rest_delta = sanitize_positive(rest_delta, DEFAULT_REST_DELTA);
        self
    }

    /// Returns a copy with a sanitized velocity rest threshold.
    pub fn with_rest_speed(mut self, rest_speed: f32) -> Self {
        self.rest_speed = sanitize_positive(rest_speed, DEFAULT_REST_SPEED);
        self
    }

    /// Returns a copy with a review-facing bounce value clamped to policy range.
    pub fn with_bounce(mut self, bounce: f32) -> Self {
        self.bounce = if bounce.is_finite() {
            bounce.clamp(0.0, Self::MAX_REVIEWABLE_BOUNCE)
        } else {
            0.0
        };
        self
    }

    /// Returns a copy with the expected review duration used by policy checks.
    pub fn with_review_duration(mut self, duration: Duration) -> Self {
        self.review_duration = duration;
        self
    }
}

/// Renderer-neutral spring motion specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringSpec {
    preference: MotionPreference,
    preset: Option<MotionSpringPreset>,
    physics: MotionSpringPhysics,
}

impl MotionSpringSpec {
    /// Creates a spring spec from preference and explicit physics.
    pub fn from_physics(preference: MotionPreference, physics: MotionSpringPhysics) -> Self {
        Self {
            preference,
            preset: None,
            physics,
        }
    }

    /// Creates the default affordance spring spec for the given preference.
    pub fn affordance(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Affordance,
            MotionSpringPhysics::new(1.0, 320.0, 34.0)
                .with_bounce(0.08)
                .with_review_duration(Duration::from_millis(120)),
        )
    }

    /// Creates the default committed layout spring spec for the given preference.
    pub fn layout(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Layout,
            MotionSpringPhysics::new(1.0, 260.0, 28.0)
                .with_bounce(0.12)
                .with_review_duration(Duration::from_millis(180)),
        )
    }

    /// Creates the default continuity spring spec for the given preference.
    pub fn continuity(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Continuity,
            MotionSpringPhysics::new(1.0, 190.0, 23.0)
                .with_bounce(0.10)
                .with_review_duration(Duration::from_millis(260)),
        )
    }

    fn from_preset(
        preference: MotionPreference,
        preset: MotionSpringPreset,
        physics: MotionSpringPhysics,
    ) -> Self {
        Self {
            preference,
            preset: Some(preset),
            physics,
        }
    }

    /// Returns the motion preference.
    pub const fn preference(self) -> MotionPreference {
        self.preference
    }

    /// Returns the optional reviewable preset.
    pub const fn preset(self) -> Option<MotionSpringPreset> {
        self.preset
    }

    /// Returns the sanitized physics parameters.
    pub const fn physics(self) -> MotionSpringPhysics {
        self.physics
    }

    /// Returns whether this spring completes immediately.
    pub const fn is_immediate(self) -> bool {
        self.preference.is_immediate()
    }

    /// Returns a copy with review-facing bounce clamped to policy range.
    pub fn with_bounce(mut self, bounce: f32) -> Self {
        self.physics = self.physics.with_bounce(bounce);
        self
    }
}

/// A sampled point on a spring transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringSample {
    state: MotionTimelineState,
    elapsed: Duration,
    value: f32,
    velocity: f32,
    target: f32,
}

impl MotionSpringSample {
    /// Creates a spring sample from explicit values.
    pub const fn new(
        state: MotionTimelineState,
        elapsed: Duration,
        value: f32,
        velocity: f32,
        target: f32,
    ) -> Self {
        Self {
            state,
            elapsed,
            value,
            velocity,
            target,
        }
    }

    /// Returns the sampled state.
    pub const fn state(self) -> MotionTimelineState {
        self.state
    }

    /// Returns the elapsed duration used for this sample.
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns the sampled scalar value.
    pub const fn value(self) -> f32 {
        self.value
    }

    /// Returns the sampled scalar velocity.
    pub const fn velocity(self) -> f32 {
        self.velocity
    }

    /// Returns the target scalar value.
    pub const fn target(self) -> f32 {
        self.target
    }

    /// Returns whether callers should continue requesting frames.
    pub const fn is_active(self) -> bool {
        self.state.is_active()
    }

    /// Returns whether the semantic final state has been reached.
    pub const fn reached_final_state(self) -> bool {
        self.state.reached_final_state()
    }
}

/// A deterministic scalar spring transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpring {
    spec: MotionSpringSpec,
    from: f32,
    target: f32,
    initial_velocity: f32,
    started_at: Instant,
    cancelled_at: Option<Instant>,
}

impl MotionSpring {
    /// Creates a scalar spring transition.
    pub fn new(
        spec: MotionSpringSpec,
        from: f32,
        target: f32,
        initial_velocity: f32,
        started_at: Instant,
    ) -> Self {
        let from = sanitize_number(from, 0.0);
        Self {
            spec,
            from,
            target: sanitize_number(target, from),
            initial_velocity: sanitize_number(initial_velocity, 0.0),
            started_at,
            cancelled_at: None,
        }
    }

    /// Creates a new spring whose source and velocity come from an interrupted sample.
    pub fn retarget_from_sample(
        spec: MotionSpringSpec,
        sample: MotionSpringSample,
        target: f32,
        started_at: Instant,
    ) -> Self {
        Self::new(spec, sample.value(), target, sample.velocity(), started_at)
    }

    /// Returns the spring specification.
    pub const fn spec(self) -> MotionSpringSpec {
        self.spec
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

    /// Returns the instant at which the spring started.
    pub const fn started_at(self) -> Instant {
        self.started_at
    }

    /// Returns the instant at which the spring was cancelled.
    pub const fn cancelled_at(self) -> Option<Instant> {
        self.cancelled_at
    }

    /// Marks the spring as cancelled at the provided instant.
    pub fn cancel_at(&mut self, cancelled_at: Instant) {
        self.cancelled_at = Some(cancelled_at);
    }

    /// Samples the spring at the provided instant.
    pub fn sample(self, now: Instant) -> MotionSpringSample {
        let effective_now = self.cancelled_at.unwrap_or(now);
        let elapsed = effective_now.saturating_duration_since(self.started_at);
        let mut sample = Self::sample_elapsed(
            self.spec,
            self.from,
            self.target,
            self.initial_velocity,
            elapsed,
        );
        if self.cancelled_at.is_some() && !sample.reached_final_state() {
            sample.state = MotionTimelineState::Cancelled;
        }
        sample
    }

    /// Samples a spring using an explicit elapsed duration.
    pub fn sample_elapsed(
        spec: MotionSpringSpec,
        from: f32,
        target: f32,
        initial_velocity: f32,
        elapsed: Duration,
    ) -> MotionSpringSample {
        let from = sanitize_number(from, 0.0);
        let target = sanitize_number(target, from);
        let initial_velocity = sanitize_number(initial_velocity, 0.0);

        if spec.is_immediate() {
            return MotionSpringSample::new(
                MotionTimelineState::Immediate,
                elapsed,
                target,
                0.0,
                target,
            );
        }

        let physics = spec.physics();
        if elapsed.is_zero() {
            let state = if at_rest(from, target, initial_velocity, physics) {
                MotionTimelineState::Completed
            } else {
                MotionTimelineState::Active
            };
            let value = if state.reached_final_state() {
                target
            } else {
                from
            };
            let velocity = if state.reached_final_state() {
                0.0
            } else {
                initial_velocity
            };
            return MotionSpringSample::new(state, elapsed, value, velocity, target);
        }

        let (value, velocity) =
            sample_spring_value(physics, from, target, initial_velocity, elapsed);
        if at_rest(value, target, velocity, physics) {
            MotionSpringSample::new(MotionTimelineState::Completed, elapsed, target, 0.0, target)
        } else {
            MotionSpringSample::new(
                MotionTimelineState::Active,
                elapsed,
                value,
                velocity,
                target,
            )
        }
    }
}

/// Shared motion model wrapper for timeline and spring transitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionModel {
    /// Duration/easing timeline motion.
    Timeline(MotionSpec),
    /// Velocity-aware spring motion.
    Spring(MotionSpringSpec),
}

impl MotionModel {
    /// Creates a timeline motion model.
    pub const fn timeline(spec: MotionSpec) -> Self {
        Self::Timeline(spec)
    }

    /// Creates a spring motion model.
    pub const fn spring(spec: MotionSpringSpec) -> Self {
        Self::Spring(spec)
    }

    /// Returns the motion preference.
    pub const fn preference(self) -> MotionPreference {
        match self {
            Self::Timeline(spec) => spec.preference(),
            Self::Spring(spec) => spec.preference(),
        }
    }

    /// Returns whether this model completes immediately.
    pub const fn is_immediate(self) -> bool {
        match self {
            Self::Timeline(spec) => spec.is_immediate(),
            Self::Spring(spec) => spec.is_immediate(),
        }
    }
}

fn sample_spring_value(
    physics: MotionSpringPhysics,
    from: f32,
    target: f32,
    initial_velocity: f32,
    elapsed: Duration,
) -> (f32, f32) {
    let t = elapsed.as_secs_f32().max(0.0);
    let displacement = from - target;
    let mass = physics.mass();
    let stiffness = physics.stiffness();
    let damping = physics.damping();
    let angular_frequency = (stiffness / mass).sqrt();
    let critical_damping = 2.0 * (stiffness * mass).sqrt();
    let damping_ratio = if critical_damping > 0.0 {
        damping / critical_damping
    } else {
        1.0
    };

    let (displacement, velocity) = if damping_ratio < 1.0 {
        sample_underdamped(
            displacement,
            initial_velocity,
            angular_frequency,
            damping_ratio,
            t,
        )
    } else if (damping_ratio - 1.0).abs() <= f32::EPSILON {
        sample_critically_damped(displacement, initial_velocity, angular_frequency, t)
    } else {
        sample_overdamped(
            displacement,
            initial_velocity,
            angular_frequency,
            damping_ratio,
            t,
        )
    };

    let value = sanitize_number(target + displacement, target);
    let velocity = sanitize_number(velocity, 0.0);
    (value, velocity)
}

fn sample_underdamped(
    displacement: f32,
    velocity: f32,
    angular_frequency: f32,
    damping_ratio: f32,
    time: f32,
) -> (f32, f32) {
    let damped_frequency = angular_frequency * (1.0 - damping_ratio * damping_ratio).sqrt();
    if damped_frequency <= MIN_POSITIVE {
        return sample_critically_damped(displacement, velocity, angular_frequency, time);
    }

    let decay = (-damping_ratio * angular_frequency * time).exp();
    let sin = (damped_frequency * time).sin();
    let cos = (damped_frequency * time).cos();
    let a = displacement;
    let b = (velocity + damping_ratio * angular_frequency * displacement) / damped_frequency;
    let oscillation = a * cos + b * sin;
    let sampled_displacement = decay * oscillation;
    let sampled_velocity = decay
        * ((-a * damped_frequency * sin + b * damped_frequency * cos)
            - damping_ratio * angular_frequency * oscillation);
    (sampled_displacement, sampled_velocity)
}

fn sample_critically_damped(
    displacement: f32,
    velocity: f32,
    angular_frequency: f32,
    time: f32,
) -> (f32, f32) {
    let decay = (-angular_frequency * time).exp();
    let c = velocity + angular_frequency * displacement;
    let displacement_term = displacement + c * time;
    let sampled_displacement = decay * displacement_term;
    let sampled_velocity = decay * (c - angular_frequency * displacement_term);
    (sampled_displacement, sampled_velocity)
}

fn sample_overdamped(
    displacement: f32,
    velocity: f32,
    angular_frequency: f32,
    damping_ratio: f32,
    time: f32,
) -> (f32, f32) {
    let ratio_delta = (damping_ratio * damping_ratio - 1.0).sqrt();
    let r1 = -angular_frequency * (damping_ratio - ratio_delta);
    let r2 = -angular_frequency * (damping_ratio + ratio_delta);
    if (r1 - r2).abs() <= MIN_POSITIVE {
        return sample_critically_damped(displacement, velocity, angular_frequency, time);
    }
    let c1 = (velocity - r2 * displacement) / (r1 - r2);
    let c2 = displacement - c1;
    let e1 = (r1 * time).exp();
    let e2 = (r2 * time).exp();
    let sampled_displacement = c1 * e1 + c2 * e2;
    let sampled_velocity = c1 * r1 * e1 + c2 * r2 * e2;
    (sampled_displacement, sampled_velocity)
}

fn at_rest(value: f32, target: f32, velocity: f32, physics: MotionSpringPhysics) -> bool {
    (value - target).abs() <= physics.rest_delta() && velocity.abs() <= physics.rest_speed()
}

fn sanitize_number(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

fn sanitize_positive(value: f32, default: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value.clamp(MIN_POSITIVE, MAX_PHYSICS_VALUE)
    } else {
        default
    }
}

fn sanitize_non_negative(value: f32, default: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value.clamp(0.0, MAX_PHYSICS_VALUE)
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotionPreference;
    use std::time::{Duration, Instant};

    #[test]
    fn layout_spring_samples_active_motion_and_reaches_exact_target_at_rest() {
        let started_at = Instant::now();
        let spring = MotionSpring::new(
            MotionSpringSpec::layout(MotionPreference::Animated),
            0.0,
            1.0,
            0.0,
            started_at,
        );

        let start = spring.sample(started_at);
        assert_eq!(start.state(), MotionTimelineState::Active);
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert_eq!(start.value(), 0.0);
        assert_eq!(start.target(), 1.0);

        let midpoint = spring.sample(started_at + Duration::from_millis(90));
        assert_eq!(midpoint.state(), MotionTimelineState::Active);
        assert!(midpoint.value() > 0.0);
        assert!(midpoint.value() < 1.08);
        assert!(midpoint.velocity().is_finite());

        let complete = spring.sample(started_at + Duration::from_secs(2));
        assert_eq!(complete.state(), MotionTimelineState::Completed);
        assert_eq!(complete.value(), 1.0);
        assert_eq!(complete.velocity(), 0.0);
        assert!(complete.reached_final_state());
    }

    #[test]
    fn tiny_delta_uses_rest_thresholds_without_oscillating_forever() {
        let sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::layout(MotionPreference::Animated),
            10.0,
            10.0001,
            0.0,
            Duration::from_millis(600),
        );

        assert_eq!(sample.state(), MotionTimelineState::Completed);
        assert_eq!(sample.value(), 10.0001);
    }

    #[test]
    fn spring_bounce_defaults_are_subtle_and_clamped() {
        let layout = MotionSpringSpec::layout(MotionPreference::Animated);
        assert!(layout.physics().bounce() <= 0.20);

        let clamped = layout.with_bounce(2.0);
        assert!(clamped.physics().bounce() <= MotionSpringPhysics::MAX_REVIEWABLE_BOUNCE);
    }

    #[test]
    fn retargeted_spring_preserves_current_position_and_velocity() {
        let started_at = Instant::now();
        let spec = MotionSpringSpec::layout(MotionPreference::Animated);
        let spring = MotionSpring::new(spec, 0.0, 1.0, 0.0, started_at);
        let sampled_at = started_at + Duration::from_millis(80);
        let sampled = spring.sample(sampled_at);

        let retargeted = MotionSpring::retarget_from_sample(spec, sampled, 2.0, sampled_at);
        let retarget_start = retargeted.sample(sampled_at);

        assert_eq!(retarget_start.value(), sampled.value());
        assert_eq!(retarget_start.velocity(), sampled.velocity());
        assert_eq!(retarget_start.target(), 2.0);
    }

    #[test]
    fn reduced_motion_spring_returns_final_semantic_sample() {
        let sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::layout(MotionPreference::Reduced),
            0.0,
            1.0,
            20.0,
            Duration::from_millis(16),
        );

        assert_eq!(sample.state(), MotionTimelineState::Immediate);
        assert_eq!(sample.value(), 1.0);
        assert_eq!(sample.velocity(), 0.0);
        assert!(sample.reached_final_state());
    }

    #[test]
    fn invalid_physics_parameters_are_sanitized() {
        let physics = MotionSpringPhysics::new(f32::NAN, -1.0, f32::INFINITY)
            .with_rest_delta(f32::NAN)
            .with_rest_speed(-20.0);
        let sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::from_physics(MotionPreference::Animated, physics),
            0.0,
            1.0,
            f32::NAN,
            Duration::from_millis(80),
        );

        assert!(sample.value().is_finite());
        assert!(sample.velocity().is_finite());
    }

    #[test]
    fn motion_model_wraps_timeline_and_spring_specs() {
        let timeline = MotionModel::timeline(crate::MotionSpec::layout(MotionPreference::Animated));
        let spring = MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated));

        assert!(!timeline.is_immediate());
        assert!(!spring.is_immediate());
        assert_eq!(spring.preference(), MotionPreference::Animated);
    }
}
