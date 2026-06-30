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
}
