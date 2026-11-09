use std::ops::RangeInclusive;

use crate::{Hsla, Pixels, Rems, Rgba};

pub use open_gpui_motion::{SpringConfig, SpringState, sampled_easing};

const DEFAULT_SPRING_EPSILON: f32 = 0.001;

/// A value that can be targeted by a one-dimensional spring.
///
/// Implementations may project the spring coordinate into a richer output,
/// allowing a discrete state or a path through a multidimensional value to be
/// driven by one spring.
pub trait SpringTarget: 'static {
    /// The value supplied to the spring animator.
    type Output;

    /// Returns the target in the spring's coordinate space.
    fn target(&self) -> f32;

    /// Projects a spring coordinate into the animated output.
    fn resolve(&self, value: f32) -> Self::Output;
}

impl SpringTarget for f32 {
    type Output = f32;

    fn target(&self) -> f32 {
        *self
    }

    fn resolve(&self, value: f32) -> Self::Output {
        value
    }
}

impl SpringTarget for Pixels {
    type Output = Pixels;

    fn target(&self) -> f32 {
        self.as_f32()
    }

    fn resolve(&self, value: f32) -> Self::Output {
        Pixels::from(value)
    }
}

impl SpringTarget for Rems {
    type Output = Rems;

    fn target(&self) -> f32 {
        self.0
    }

    fn resolve(&self, value: f32) -> Self::Output {
        Rems(value)
    }
}

impl SpringTarget for bool {
    type Output = AnimationPhase;

    fn target(&self) -> f32 {
        if *self { 1.0 } else { 0.0 }
    }

    fn resolve(&self, value: f32) -> Self::Output {
        AnimationPhase(value)
    }
}

/// A potentially overshooting coordinate within an animation.
///
/// Phases are not restricted to 0..1. Multi-stage animations can assign each
/// stage its own coordinate and interpolate over ranges such as 1..=2.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct AnimationPhase(
    /// The unbounded phase coordinate.
    pub f32,
);

impl AnimationPhase {
    /// Restricts this phase to the bounds of a range.
    pub fn clamp(self, range: RangeInclusive<f32>) -> Self {
        let (first, second) = range.into_inner();
        Self(self.0.clamp(first.min(second), first.max(second)))
    }

    /// Interpolates between values using 0 and 1 as their phase coordinates.
    pub fn interpolate<T: Interpolate>(self, from: T, to: T) -> T {
        T::interpolate(from, to, self.0)
    }

    /// Interpolates between values without extrapolating beyond 0 and 1.
    pub fn interpolate_clamped<T: Interpolate>(self, from: T, to: T) -> T {
        T::interpolate(from, to, self.0.clamp(0.0, 1.0))
    }

    /// Interpolates between values assigned to arbitrary phase coordinates.
    pub fn interpolate_between<T: Interpolate>(
        self,
        range: RangeInclusive<f32>,
        from: T,
        to: T,
    ) -> T {
        let (start, end) = range.into_inner();
        let phase = if start == end {
            if self.0 < start { 0.0 } else { 1.0 }
        } else {
            (self.0 - start) / (end - start)
        };
        T::interpolate(from, to, phase)
    }

    /// Interpolates over arbitrary phase coordinates without extrapolating.
    pub fn interpolate_between_clamped<T: Interpolate>(
        self,
        range: RangeInclusive<f32>,
        from: T,
        to: T,
    ) -> T {
        let (start, end) = range.into_inner();
        let phase = if start == end {
            if self.0 < start { 0.0 } else { 1.0 }
        } else {
            ((self.0 - start) / (end - start)).clamp(0.0, 1.0)
        };
        T::interpolate(from, to, phase)
    }
}

impl From<f32> for AnimationPhase {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<bool> for AnimationPhase {
    fn from(value: bool) -> Self {
        Self(if value { 1.0 } else { 0.0 })
    }
}

impl SpringTarget for AnimationPhase {
    type Output = AnimationPhase;

    fn target(&self) -> f32 {
        self.0
    }

    fn resolve(&self, value: f32) -> Self::Output {
        Self(value)
    }
}

/// A value that supports linear interpolation and extrapolation.
pub trait Interpolate: Sized {
    /// Resolves the value at `phase`, where 0 is `from` and 1 is `to`.
    fn interpolate(from: Self, to: Self, phase: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(from: Self, to: Self, phase: f32) -> Self {
        from + (to - from) * phase
    }
}

impl Interpolate for Pixels {
    fn interpolate(from: Self, to: Self, phase: f32) -> Self {
        from + (to - from) * phase
    }
}

impl Interpolate for Rems {
    fn interpolate(from: Self, to: Self, phase: f32) -> Self {
        from + (to - from) * phase
    }
}

impl Interpolate for Rgba {
    fn interpolate(from: Self, to: Self, phase: f32) -> Self {
        Self {
            r: f32::interpolate(from.r, to.r, phase),
            g: f32::interpolate(from.g, to.g, phase),
            b: f32::interpolate(from.b, to.b, phase),
            a: f32::interpolate(from.a, to.a, phase),
        }
    }
}

impl Interpolate for Hsla {
    fn interpolate(from: Self, to: Self, phase: f32) -> Self {
        let hue_delta = (to.h - from.h + 0.5).rem_euclid(1.0) - 0.5;
        Self {
            h: (from.h + hue_delta * phase).rem_euclid(1.0),
            s: f32::interpolate(from.s, to.s, phase),
            l: f32::interpolate(from.l, to.l, phase),
            a: f32::interpolate(from.a, to.a, phase),
        }
    }
}

/// Controls how a spring advances and resolves its presentation value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpringPlayback {
    /// Advances toward the latest target, preserving velocity across retargets.
    #[default]
    Running,
    /// Holds the current position and velocity until playback resumes.
    Paused,
    /// Holds the current position and discards velocity.
    Stopped,
    /// Snaps to the latest target and discards velocity.
    Completed,
    /// Returns to the initial value and discards velocity.
    Cancelled,
}

/// A stateful spring animation targeting a value or projected path.
#[derive(Clone, Debug)]
pub struct SpringAnimation<T = ()> {
    pub(crate) config: SpringConfig,
    pub(crate) target: T,
    pub(crate) epsilon: f32,
    pub(crate) initial: Option<f32>,
    pub(crate) playback: SpringPlayback,
}

impl SpringAnimation<()> {
    /// Creates a spring animation builder.
    pub fn new(config: SpringConfig) -> Self {
        Self {
            config,
            target: (),
            epsilon: DEFAULT_SPRING_EPSILON,
            initial: None,
            playback: SpringPlayback::Running,
        }
    }

    /// Sets the value or path targeted by this spring.
    pub fn to<T: SpringTarget>(self, target: T) -> SpringAnimation<T> {
        let SpringAnimation {
            config,
            target: (),
            epsilon,
            initial,
            playback,
        } = self;
        SpringAnimation {
            config,
            target,
            epsilon,
            initial,
            playback,
        }
    }
}

impl<T> SpringAnimation<T> {
    /// Sets the settling tolerance in the target's scalar coordinate space.
    pub fn with_epsilon(mut self, epsilon: f32) -> Self {
        self.epsilon = epsilon;
        self
    }

    /// Sets how the spring advances or resolves its current value.
    pub fn playback(mut self, playback: SpringPlayback) -> Self {
        self.playback = playback;
        self
    }
}

impl<T: SpringTarget> SpringAnimation<T> {
    /// Sets the coordinate used when this element has no prior spring state.
    pub fn from(mut self, initial: T) -> Self {
        self.initial = Some(initial.target());
        self
    }
}

#[cfg(test)]
mod tests;
