//! Renderer-neutral scalar motion value state.

/// Renderer-neutral sanitized scalar value consumed by motion tracks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MotionValue {
    current: f32,
}

impl MotionValue {
    /// Creates a scalar value.
    pub(crate) fn new(value: f32) -> Self {
        Self {
            current: sanitize(value, 0.0),
        }
    }

    /// Returns the current scalar value.
    pub(crate) const fn current(&self) -> f32 {
        self.current
    }
}

fn sanitize(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_value_keeps_finite_current_value() {
        assert_eq!(MotionValue::new(12.5).current(), 12.5);
    }

    #[test]
    fn scalar_value_replaces_non_finite_current_value_with_zero() {
        assert_eq!(MotionValue::new(f32::NAN).current(), 0.0);
        assert_eq!(MotionValue::new(f32::INFINITY).current(), 0.0);
    }
}
