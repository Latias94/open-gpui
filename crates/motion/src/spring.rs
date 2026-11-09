//! Renderer-neutral spring sampling for layout-like UI motion.

use crate::{
    MotionPreference, MotionRunState, SpringConfig, SpringState, motion::MotionSpec,
    runtime::MotionTimeline,
};
use std::time::Duration;

const DEFAULT_SPRING_EPSILON: f32 = 0.001;

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

/// Review hints used by domain motion policy, separate from spring physics.
///
/// Neither hint changes the solver. Reported bounce is not measured overshoot, and the review
/// duration is not the spring's physical settling time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringPolicyHints {
    reported_bounce: f32,
    review_duration: Duration,
}

impl MotionSpringPolicyHints {
    /// Maximum bounce value accepted by professional UI policy.
    pub const MAX_REVIEWABLE_BOUNCE: f32 = 0.35;

    /// Creates review hints, clamping finite reported bounce to the policy range.
    ///
    /// Non-finite reported bounce becomes zero. The duration is retained for policy validation;
    /// an over-budget duration is reported by the validator rather than shortened here.
    pub fn new(review_duration: Duration, reported_bounce: f32) -> Self {
        Self {
            reported_bounce: if reported_bounce.is_finite() {
                reported_bounce.clamp(0.0, Self::MAX_REVIEWABLE_BOUNCE)
            } else {
                0.0
            },
            review_duration,
        }
    }

    /// Returns the review-facing bounce hint.
    pub const fn reported_bounce(self) -> f32 {
        self.reported_bounce
    }

    /// Returns the expected review duration for policy checks.
    pub const fn review_duration(self) -> Duration {
        self.review_duration
    }
}

/// Domain sampling options using the shared upstream spring configuration.
///
/// The configuration is retained exactly and must satisfy [`SpringConfig`]'s preconditions.
/// Settling uses one positional epsilon and the solver's natural-frequency velocity rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringOptions {
    config: SpringConfig,
    epsilon: f32,
    policy_hints: MotionSpringPolicyHints,
}

impl MotionSpringOptions {
    /// Creates domain options without altering the supplied physical configuration.
    pub const fn new(config: SpringConfig) -> Self {
        Self {
            config,
            epsilon: DEFAULT_SPRING_EPSILON,
            policy_hints: MotionSpringPolicyHints {
                reported_bounce: 0.0,
                review_duration: Duration::from_millis(180),
            },
        }
    }

    /// Returns the exact shared physical configuration.
    pub const fn config(self) -> SpringConfig {
        self.config
    }

    /// Returns the positional settling tolerance.
    pub const fn epsilon(self) -> f32 {
        self.epsilon
    }

    /// Returns the domain review hints that do not participate in the solver.
    pub const fn policy_hints(self) -> MotionSpringPolicyHints {
        self.policy_hints
    }

    /// Returns a copy with a finite, non-negative positional tolerance.
    ///
    /// Negative or non-finite input uses the default tolerance of `0.001`. Zero is accepted,
    /// following the solver's exact-rest rule.
    pub fn with_epsilon(mut self, epsilon: f32) -> Self {
        self.epsilon = if epsilon.is_finite() && epsilon >= 0.0 {
            epsilon
        } else {
            DEFAULT_SPRING_EPSILON
        };
        self
    }

    /// Returns a copy with domain review hints.
    pub const fn with_policy_hints(mut self, policy_hints: MotionSpringPolicyHints) -> Self {
        self.policy_hints = policy_hints;
        self
    }
}

/// Domain spring specification with explicit motion preference and review policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpringSpec {
    preference: MotionPreference,
    preset: Option<MotionSpringPreset>,
    options: MotionSpringOptions,
}

impl MotionSpringSpec {
    /// Creates a spring spec from preference and explicit domain options.
    pub const fn from_options(preference: MotionPreference, options: MotionSpringOptions) -> Self {
        Self {
            preference,
            preset: None,
            options,
        }
    }

    /// Creates the default affordance spring spec for the given preference.
    pub fn affordance(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Affordance,
            MotionSpringOptions::new(SpringConfig::new(320.0, 34.0, 1.0)).with_policy_hints(
                MotionSpringPolicyHints::new(Duration::from_millis(120), 0.08),
            ),
        )
    }

    /// Creates the default committed layout spring spec for the given preference.
    pub fn layout(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Layout,
            MotionSpringOptions::new(SpringConfig::new(260.0, 28.0, 1.0)).with_policy_hints(
                MotionSpringPolicyHints::new(Duration::from_millis(180), 0.12),
            ),
        )
    }

    /// Creates the default continuity spring spec for the given preference.
    pub fn continuity(preference: MotionPreference) -> Self {
        Self::from_preset(
            preference,
            MotionSpringPreset::Continuity,
            MotionSpringOptions::new(SpringConfig::new(190.0, 23.0, 1.0)).with_policy_hints(
                MotionSpringPolicyHints::new(Duration::from_millis(260), 0.10),
            ),
        )
    }

    fn from_preset(
        preference: MotionPreference,
        preset: MotionSpringPreset,
        options: MotionSpringOptions,
    ) -> Self {
        Self {
            preference,
            preset: Some(preset),
            options,
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

    /// Returns the shared solver configuration, epsilon and separate domain review hints.
    pub const fn options(self) -> MotionSpringOptions {
        self.options
    }

    /// Returns whether this spring completes immediately.
    pub const fn is_immediate(self) -> bool {
        self.preference.is_immediate()
    }

    /// Returns a copy with domain review hints.
    pub const fn with_policy_hints(mut self, policy_hints: MotionSpringPolicyHints) -> Self {
        self.options = self.options.with_policy_hints(policy_hints);
        self
    }
}

/// Explicit preset that resolves to a concrete motion model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionPreset {
    /// Immediate completion.
    Immediate,
    /// Duration/easing timeline motion.
    Timeline(MotionSpec),
    /// Explicit spring motion.
    Spring(MotionSpringSpec),
    /// Default committed layout spring.
    CommittedLayout(MotionPreference),
    /// Default continuity spring.
    Continuity(MotionPreference),
    /// Default affordance spring.
    Affordance(MotionPreference),
}

impl MotionPreset {
    /// Creates an immediate preset.
    pub const fn immediate() -> Self {
        Self::Immediate
    }

    /// Creates an explicit timeline preset.
    pub const fn timeline(spec: MotionSpec) -> Self {
        Self::Timeline(spec)
    }

    /// Creates an explicit spring preset.
    pub const fn spring(spec: MotionSpringSpec) -> Self {
        Self::Spring(spec)
    }

    /// Creates the default committed layout preset.
    pub const fn committed_layout(preference: MotionPreference) -> Self {
        Self::CommittedLayout(preference)
    }

    /// Creates the default continuity preset.
    pub const fn continuity(preference: MotionPreference) -> Self {
        Self::Continuity(preference)
    }

    /// Creates the default affordance preset.
    pub const fn affordance(preference: MotionPreference) -> Self {
        Self::Affordance(preference)
    }

    /// Resolves this preset to the concrete motion model that should execute.
    pub fn resolve_model(self) -> MotionModel {
        match self {
            Self::Immediate => MotionModel::timeline(MotionSpec::immediate()),
            Self::Timeline(spec) => MotionModel::timeline(spec),
            Self::Spring(spec) => MotionModel::spring(spec),
            Self::CommittedLayout(preference) => {
                MotionModel::spring(MotionSpringSpec::layout(preference))
            }
            Self::Continuity(preference) => {
                MotionModel::spring(MotionSpringSpec::continuity(preference))
            }
            Self::Affordance(preference) => {
                MotionModel::spring(MotionSpringSpec::affordance(preference))
            }
        }
    }
}

/// A sampled point on a scalar motion model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionScalarSample {
    state: MotionRunState,
    elapsed: Duration,
    value: f32,
    velocity: f32,
    target: f32,
}

impl MotionScalarSample {
    /// Creates a scalar motion sample from explicit values.
    pub const fn new(
        state: MotionRunState,
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
    pub const fn state(self) -> MotionRunState {
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

/// Deterministic scalar spring sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MotionSpring;

impl MotionSpring {
    /// Samples a spring using an explicit elapsed duration.
    pub(crate) fn sample_elapsed(
        spec: MotionSpringSpec,
        from: f32,
        target: f32,
        initial_velocity: f32,
        elapsed: Duration,
    ) -> MotionScalarSample {
        let from = sanitize_number(from, 0.0);
        let target = sanitize_number(target, from);
        let initial_velocity = sanitize_number(initial_velocity, 0.0);

        if spec.is_immediate() {
            return MotionScalarSample::new(
                MotionRunState::Immediate,
                elapsed,
                target,
                0.0,
                target,
            );
        }

        let options = spec.options();
        let config = options.config();
        let initial = SpringState {
            position: from,
            velocity: initial_velocity,
        };
        let sampled = if elapsed.is_zero() {
            initial
        } else {
            config.step(initial, target, elapsed.as_secs_f32())
        };
        if config.is_settled(sampled, target, options.epsilon()) {
            MotionScalarSample::new(MotionRunState::Completed, elapsed, target, 0.0, target)
        } else {
            MotionScalarSample::new(
                MotionRunState::Active,
                elapsed,
                sampled.position,
                sampled.velocity,
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

    /// Returns the duration used when this model participates in a sequence plan.
    ///
    /// Timeline models use their exact duration. Spring models use their review-duration hint,
    /// because physical completion is still determined by sampled rest state.
    pub const fn sequence_duration_hint(self) -> Duration {
        match self {
            Self::Timeline(spec) => spec.duration().as_duration(),
            Self::Spring(spec) => spec.options().policy_hints().review_duration(),
        }
    }

    /// Samples this motion model as a scalar value.
    ///
    /// Non-finite source, target and initial velocity use zero, the source and zero respectively.
    /// Spring configurations must satisfy [`SpringConfig`]'s physical preconditions. Active spring
    /// samples preserve the shared solver's position and velocity without clamping overshoot.
    pub fn sample_scalar_elapsed(
        self,
        from: f32,
        target: f32,
        initial_velocity: f32,
        elapsed: Duration,
    ) -> MotionScalarSample {
        match self {
            Self::Timeline(spec) => {
                let sample = MotionTimeline::sample_elapsed(spec, elapsed);
                let from = sanitize_number(from, 0.0);
                let target = sanitize_number(target, from);
                let value = from + (target - from) * sample.progress();
                let duration = spec.duration().as_duration().as_secs_f32();
                let velocity = if sample.reached_final_state() || duration <= 0.0 {
                    0.0
                } else {
                    (target - from) / duration
                };
                MotionScalarSample::new(sample.state(), sample.elapsed(), value, velocity, target)
            }
            Self::Spring(spec) => {
                MotionSpring::sample_elapsed(spec, from, target, initial_velocity, elapsed)
            }
        }
    }
}

fn sanitize_number(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotionPreference;
    use std::time::Duration;

    #[test]
    fn layout_spring_samples_active_motion_and_reaches_exact_target_at_rest() {
        let spec = MotionSpringSpec::layout(MotionPreference::Animated);
        let start = MotionSpring::sample_elapsed(spec, 0.0, 1.0, 0.0, Duration::ZERO);
        assert_eq!(start.state(), MotionRunState::Active);
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert_eq!(start.value(), 0.0);
        assert_eq!(start.target(), 1.0);

        let midpoint = MotionSpring::sample_elapsed(spec, 0.0, 1.0, 0.0, Duration::from_millis(90));
        assert_eq!(midpoint.state(), MotionRunState::Active);
        assert!(midpoint.value() > 0.0);
        assert!(midpoint.value() < 1.08);
        assert!(midpoint.velocity().is_finite());

        let complete = MotionSpring::sample_elapsed(spec, 0.0, 1.0, 0.0, Duration::from_secs(2));
        assert_eq!(complete.state(), MotionRunState::Completed);
        assert_eq!(complete.value(), 1.0);
        assert_eq!(complete.velocity(), 0.0);
        assert!(complete.reached_final_state());
    }

    #[test]
    fn spring_samples_can_seed_retargeted_elapsed_runs() {
        let spec = MotionSpringSpec::layout(MotionPreference::Animated);
        let sampled = MotionSpring::sample_elapsed(spec, 0.0, 1.0, 0.0, Duration::from_millis(80));

        let retarget_start = MotionSpring::sample_elapsed(
            spec,
            sampled.value(),
            2.0,
            sampled.velocity(),
            Duration::ZERO,
        );

        assert_eq!(retarget_start.value(), sampled.value());
        assert_eq!(retarget_start.velocity(), sampled.velocity());
        assert_eq!(retarget_start.target(), 2.0);
    }

    #[test]
    fn cancelled_motion_is_a_controller_state_not_a_spring_lifecycle() {
        let mut sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::layout(MotionPreference::Animated),
            0.0,
            1.0,
            0.0,
            Duration::from_millis(80),
        );

        sample.state = MotionRunState::Cancelled;

        assert_eq!(sample.state(), MotionRunState::Cancelled);
        assert!(!sample.reached_final_state());
        assert!(sample.value() > 0.0);
    }

    #[test]
    fn tiny_delta_uses_shared_settling_rule_without_oscillating_forever() {
        let sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::layout(MotionPreference::Animated),
            10.0,
            10.0001,
            0.0,
            Duration::from_millis(600),
        );

        assert_eq!(sample.state(), MotionRunState::Completed);
        assert_eq!(sample.value(), 10.0001);
    }

    #[test]
    fn spring_review_bounce_defaults_are_subtle_and_clamped() {
        let layout = MotionSpringSpec::layout(MotionPreference::Animated);
        assert!(layout.options().policy_hints().reported_bounce() <= 0.20);

        let clamped = layout.with_policy_hints(MotionSpringPolicyHints::new(
            Duration::from_millis(180),
            2.0,
        ));
        assert!(
            clamped.options().policy_hints().reported_bounce()
                <= MotionSpringPolicyHints::MAX_REVIEWABLE_BOUNCE
        );
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

        assert_eq!(sample.state(), MotionRunState::Immediate);
        assert_eq!(sample.value(), 1.0);
        assert_eq!(sample.velocity(), 0.0);
        assert!(sample.reached_final_state());
    }

    #[test]
    fn domain_policy_inputs_are_sanitized_without_changing_physical_configuration() {
        let config = SpringConfig::new(260.0, 28.0, 1.0);
        let options = MotionSpringOptions::new(config)
            .with_epsilon(f32::NAN)
            .with_policy_hints(MotionSpringPolicyHints::new(
                Duration::from_secs(2),
                f32::INFINITY,
            ));
        assert_eq!(options.config(), config);
        assert_eq!(options.epsilon(), DEFAULT_SPRING_EPSILON);
        assert_eq!(options.policy_hints().reported_bounce(), 0.0);
        assert_eq!(
            options.policy_hints().review_duration(),
            Duration::from_secs(2)
        );
        assert_eq!(
            options.with_epsilon(-20.0).epsilon(),
            DEFAULT_SPRING_EPSILON
        );
        assert_eq!(options.with_epsilon(0.0).epsilon(), 0.0);

        let sample = MotionSpring::sample_elapsed(
            MotionSpringSpec::from_options(MotionPreference::Animated, options),
            0.0,
            1.0,
            f32::NAN,
            Duration::from_millis(80),
        );

        assert!(sample.value().is_finite());
        assert!(sample.velocity().is_finite());
    }

    #[test]
    fn domain_options_preserve_the_upstream_configuration_preconditions() {
        let config = SpringConfig::new(-1.0, f32::INFINITY, 0.0);
        let retained = MotionSpringOptions::new(config).config();
        assert_eq!(retained, config);
    }

    #[test]
    fn domain_sampling_preserves_shared_solver_values_in_every_damping_regime() {
        let initial = SpringState {
            position: -3.0,
            velocity: 5.0,
        };
        let target = 7.0;
        for damping in [0.0, 4.0, 20.0, 40.0] {
            let config = SpringConfig::new(100.0, damping, 1.0);
            let epsilon = 0.001;
            let spec = MotionSpringSpec::from_options(
                MotionPreference::Animated,
                MotionSpringOptions::new(config).with_epsilon(epsilon),
            );
            for elapsed in [Duration::from_millis(34), Duration::from_secs(4)] {
                let expected = config.step(initial, target, elapsed.as_secs_f32());
                let sample = MotionSpring::sample_elapsed(
                    spec,
                    initial.position,
                    target,
                    initial.velocity,
                    elapsed,
                );
                if config.is_settled(expected, target, epsilon) {
                    assert_eq!(sample.state(), MotionRunState::Completed);
                    assert_eq!(sample.value(), target);
                    assert_eq!(sample.velocity(), 0.0);
                } else {
                    assert_eq!(sample.state(), MotionRunState::Active);
                    assert_eq!(sample.value(), expected.position);
                    assert_eq!(sample.velocity(), expected.velocity);
                }
            }
        }
    }

    #[test]
    fn domain_settling_uses_the_natural_frequency_velocity_threshold() {
        for (stiffness, expected_state) in [
            (25.0, MotionRunState::Active),
            (100.0, MotionRunState::Completed),
        ] {
            let spec = MotionSpringSpec::from_options(
                MotionPreference::Animated,
                MotionSpringOptions::new(SpringConfig::new(stiffness, 10.0, 1.0))
                    .with_epsilon(0.01),
            );
            let sample = MotionSpring::sample_elapsed(spec, 1.0, 1.0, 0.075, Duration::ZERO);
            assert_eq!(sample.state(), expected_state);
            if sample.is_active() {
                assert_eq!(sample.velocity(), 0.075);
            } else {
                assert_eq!(sample.velocity(), 0.0);
            }
        }
    }

    #[test]
    fn policy_hints_do_not_change_spring_samples() {
        let original = MotionSpringSpec::layout(MotionPreference::Animated);
        let altered = original.with_policy_hints(MotionSpringPolicyHints::new(
            Duration::from_secs(20),
            MotionSpringPolicyHints::MAX_REVIEWABLE_BOUNCE,
        ));
        for elapsed in [Duration::from_millis(90), Duration::from_secs(2)] {
            assert_eq!(
                MotionSpring::sample_elapsed(original, 0.0, 1.0, 0.0, elapsed),
                MotionSpring::sample_elapsed(altered, 0.0, 1.0, 0.0, elapsed)
            );
        }
    }

    #[test]
    fn motion_model_wraps_timeline_and_spring_specs() {
        let timeline = MotionModel::timeline(MotionSpec::layout(MotionPreference::Animated));
        let spring = MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated));

        assert!(!timeline.is_immediate());
        assert!(!spring.is_immediate());
        assert_eq!(spring.preference(), MotionPreference::Animated);
    }

    #[test]
    fn motion_preset_resolves_default_springs_explicitly() {
        let custom_timeline = MotionPreset::timeline(MotionSpec::new(
            MotionPreference::Animated,
            crate::MotionDuration::Custom(Duration::from_millis(240)),
            crate::MotionEasing::Linear,
        ))
        .resolve_model();
        assert!(matches!(custom_timeline, MotionModel::Timeline(_)));

        let committed = MotionPreset::committed_layout(MotionPreference::Animated).resolve_model();
        assert!(matches!(
            committed,
            MotionModel::Spring(spec)
                if spec.preset() == Some(MotionSpringPreset::Layout)
        ));

        let continuity = MotionPreset::continuity(MotionPreference::Animated).resolve_model();
        assert!(matches!(
            continuity,
            MotionModel::Spring(spec)
                if spec.preset() == Some(MotionSpringPreset::Continuity)
        ));
    }

    #[test]
    fn motion_model_samples_timeline_and_spring_as_scalar_values() {
        let timeline = MotionModel::timeline(MotionSpec::new(
            MotionPreference::Animated,
            crate::MotionDuration::Custom(Duration::from_millis(200)),
            crate::MotionEasing::Linear,
        ));
        let timeline_sample =
            timeline.sample_scalar_elapsed(0.0, 10.0, 0.0, Duration::from_millis(100));

        assert_eq!(timeline_sample.state(), MotionRunState::Active);
        assert_eq!(timeline_sample.value(), 5.0);
        assert_eq!(timeline_sample.target(), 10.0);

        let spring = MotionModel::spring(MotionSpringSpec::layout(MotionPreference::Animated));
        let spring_sample =
            spring.sample_scalar_elapsed(0.0, 10.0, 0.0, Duration::from_millis(100));

        assert_eq!(spring_sample.state(), MotionRunState::Active);
        assert!(spring_sample.value() > 0.0);
        assert_eq!(spring_sample.target(), 10.0);
        assert!(spring_sample.velocity().is_finite());
    }
}
