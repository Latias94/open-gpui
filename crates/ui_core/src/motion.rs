//! Renderer-neutral motion descriptors.

use std::time::Duration;

/// User or application preference for transition execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPreference {
    /// Transitions may animate.
    Animated,
    /// Transitions should complete immediately while preserving final semantics.
    Reduced,
}

impl MotionPreference {
    /// Returns whether transitions should complete immediately.
    pub const fn is_immediate(self) -> bool {
        matches!(self, Self::Reduced)
    }
}

/// Semantic duration bucket for UI motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDuration {
    /// Immediate completion.
    Immediate,
    /// Short affordance motion.
    Short,
    /// Medium layout motion.
    Medium,
    /// Long emphasis motion.
    Long,
    /// Explicit duration.
    Custom(Duration),
}

impl MotionDuration {
    /// Returns the concrete duration represented by this token.
    pub const fn as_duration(self) -> Duration {
        match self {
            Self::Immediate => Duration::from_millis(0),
            Self::Short => Duration::from_millis(120),
            Self::Medium => Duration::from_millis(180),
            Self::Long => Duration::from_millis(260),
            Self::Custom(duration) => duration,
        }
    }
}

/// Renderer-neutral easing token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionEasing {
    /// Linear interpolation.
    Linear,
    /// Standard ease-out layout motion.
    EaseOut,
    /// Standard ease-in-out motion.
    EaseInOut,
    /// Strong ease-out motion for committed layout transitions.
    EaseOutStrong,
    /// Strong ease-in-out motion for zoom and continuity transitions.
    EaseInOutStrong,
}

impl MotionEasing {
    /// Samples this easing curve at a clamped unit progress.
    pub fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseOut => 1.0 - (1.0 - progress).powi(3),
            Self::EaseInOut => {
                if progress < 0.5 {
                    4.0 * progress.powi(3)
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseOutStrong => 1.0 - (1.0 - progress).powi(4),
            Self::EaseInOutStrong => {
                if progress < 0.5 {
                    8.0 * progress.powi(4)
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(4) / 2.0
                }
            }
        }
    }
}

/// A renderer-neutral transition specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSpec {
    preference: MotionPreference,
    duration: MotionDuration,
    easing: MotionEasing,
}

impl MotionSpec {
    /// Creates a motion spec from preference, duration, and easing.
    pub const fn new(
        preference: MotionPreference,
        duration: MotionDuration,
        easing: MotionEasing,
    ) -> Self {
        Self {
            preference,
            duration,
            easing,
        }
    }

    /// Creates the default layout motion spec for the given preference.
    pub const fn layout(preference: MotionPreference) -> Self {
        Self::new(preference, MotionDuration::Medium, MotionEasing::EaseOut)
    }

    /// Creates committed layout motion for insert, remove, collapse, and expand transitions.
    pub const fn committed_layout(preference: MotionPreference) -> Self {
        Self::new(
            preference,
            MotionDuration::Medium,
            MotionEasing::EaseOutStrong,
        )
    }

    /// Creates continuity motion for zoom, unzoom, and retargeted transitions.
    pub const fn continuity(preference: MotionPreference) -> Self {
        Self::new(
            preference,
            MotionDuration::Long,
            MotionEasing::EaseInOutStrong,
        )
    }

    /// Creates an immediate motion spec.
    pub const fn immediate() -> Self {
        Self::new(
            MotionPreference::Reduced,
            MotionDuration::Immediate,
            MotionEasing::Linear,
        )
    }

    /// Returns the motion preference.
    pub const fn preference(self) -> MotionPreference {
        self.preference
    }

    /// Returns the duration token.
    pub const fn duration(self) -> MotionDuration {
        if self.preference.is_immediate() {
            MotionDuration::Immediate
        } else {
            self.duration
        }
    }

    /// Returns the easing token.
    pub const fn easing(self) -> MotionEasing {
        self.easing
    }

    /// Returns whether this spec completes immediately.
    pub const fn is_immediate(self) -> bool {
        self.preference.is_immediate() || matches!(self.duration(), MotionDuration::Immediate)
    }

    /// Returns whether this spec allows spatial movement.
    pub const fn allows_spatial_motion(self) -> bool {
        !self.is_immediate()
    }

    /// Samples this motion spec's eased progress for the elapsed duration.
    pub fn progress_at(self, elapsed: Duration) -> f32 {
        if self.is_immediate() {
            return 1.0;
        }
        let duration = self.duration().as_duration();
        if duration.is_zero() {
            return 1.0;
        }
        let raw = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        self.easing().sample(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduced_motion_forces_immediate_duration() {
        let spec = MotionSpec::layout(MotionPreference::Reduced);

        assert!(spec.is_immediate());
        assert_eq!(spec.duration(), MotionDuration::Immediate);
        assert_eq!(spec.duration().as_duration(), Duration::from_millis(0));
    }

    #[test]
    fn animated_motion_preserves_duration_and_easing() {
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Long,
            MotionEasing::EaseInOut,
        );

        assert!(!spec.is_immediate());
        assert_eq!(spec.duration(), MotionDuration::Long);
        assert_eq!(spec.easing(), MotionEasing::EaseInOut);
        assert_eq!(spec.duration().as_duration(), Duration::from_millis(260));
    }

    #[test]
    fn progress_at_samples_duration_and_curve() {
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );

        assert_eq!(spec.progress_at(Duration::from_millis(0)), 0.0);
        assert_eq!(spec.progress_at(Duration::from_millis(100)), 0.5);
        assert_eq!(spec.progress_at(Duration::from_millis(250)), 1.0);
    }

    #[test]
    fn named_layout_specs_use_stronger_curves_without_changing_layout_default() {
        assert_eq!(
            MotionSpec::layout(MotionPreference::Animated).easing(),
            MotionEasing::EaseOut
        );
        assert_eq!(
            MotionSpec::committed_layout(MotionPreference::Animated).easing(),
            MotionEasing::EaseOutStrong
        );
        assert_eq!(
            MotionSpec::continuity(MotionPreference::Animated).easing(),
            MotionEasing::EaseInOutStrong
        );
    }

    #[test]
    fn reduced_motion_disables_spatial_motion() {
        assert!(!MotionSpec::layout(MotionPreference::Reduced).allows_spatial_motion());
        assert!(MotionSpec::layout(MotionPreference::Animated).allows_spatial_motion());
    }

    #[test]
    fn strong_motion_curves_are_monotonic_and_complete() {
        for easing in [MotionEasing::EaseOutStrong, MotionEasing::EaseInOutStrong] {
            let samples = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0]
                .into_iter()
                .map(|progress| easing.sample(progress))
                .collect::<Vec<_>>();

            assert_eq!(samples.first().copied(), Some(0.0));
            assert_eq!(samples.last().copied(), Some(1.0));
            assert!(
                samples.windows(2).all(|window| window[0] <= window[1]),
                "{easing:?} should be monotonic: {samples:?}"
            );
        }
    }
}
