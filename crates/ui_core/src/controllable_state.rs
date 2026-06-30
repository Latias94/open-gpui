//! Renderer-neutral helpers for controlled and uncontrolled state resolution.

/// Resolved controllable state for values that may be owned by the caller or internally seeded.
#[derive(Debug, Clone, PartialEq)]
pub struct ControllableState<T> {
    value: T,
    controlled: bool,
}

impl<T> ControllableState<T> {
    /// Creates a controlled state.
    pub const fn controlled(value: T) -> Self {
        Self {
            value,
            controlled: true,
        }
    }

    /// Creates an uncontrolled state.
    pub const fn uncontrolled(value: T) -> Self {
        Self {
            value,
            controlled: false,
        }
    }

    /// Resolves a controlled state from an optional override and default seed.
    pub fn resolve(controlled: Option<T>, default_value: impl FnOnce() -> T) -> Self {
        match controlled {
            Some(value) => Self::controlled(value),
            None => Self::uncontrolled(default_value()),
        }
    }

    /// Returns whether the resolved value is controlled by the caller.
    pub const fn is_controlled(&self) -> bool {
        self.controlled
    }

    /// Returns the resolved value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Consumes the state and returns the resolved value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Maps the resolved value while preserving the control mode.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ControllableState<U> {
        ControllableState {
            value: f(self.value),
            controlled: self.controlled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ControllableState;
    use std::cell::Cell;

    #[test]
    fn resolve_prefers_controlled_value() {
        let state =
            ControllableState::resolve(Some(String::from("explicit")), || String::from("default"));

        assert!(state.is_controlled());
        assert_eq!(state.value(), "explicit");
    }

    #[test]
    fn resolve_uses_default_once_when_uncontrolled() {
        let calls = Cell::new(0);
        let state = ControllableState::resolve(None::<String>, || {
            calls.set(calls.get() + 1);
            String::from("default")
        });

        assert!(!state.is_controlled());
        assert_eq!(state.value(), "default");
        assert_eq!(calls.get(), 1);
    }
}
