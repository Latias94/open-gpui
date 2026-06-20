//! Focus and accessibility foundation page metadata.

use open_gpui_ui_core::{Role, Toggled};

/// Page title.
pub const TITLE: &str = "Focus & A11y";
/// Page summary.
pub const SUMMARY: &str = "Focus handles and accessibility roles exposed at the foundation layer.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "FocusHandle",
    "Focusable",
    "Role::Button",
    "AccessibleAction",
    "Toggled",
];

/// One focusable control row in the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusControlSpec {
    /// Stable control id.
    pub id: &'static str,
    /// User-facing label.
    pub label: &'static str,
    /// Tab index used by the focus handle.
    pub tab_index: usize,
    /// Accessibility role used by the rendered control.
    pub role: Role,
}

impl FocusControlSpec {
    const fn new(id: &'static str, label: &'static str, tab_index: usize, role: Role) -> Self {
        Self {
            id,
            label,
            tab_index,
            role,
        }
    }
}

/// Canonical focusable controls used by the demo.
pub const FOCUS_CONTROLS: [FocusControlSpec; 3] = [
    FocusControlSpec::new("focus-primary", "Primary action", 1, Role::Button),
    FocusControlSpec::new("focus-counter", "Counter", 2, Role::SpinButton),
    FocusControlSpec::new("focus-switch", "Feature switch", 3, Role::Switch),
];

/// Mutable shell state for the focus and accessibility page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FocusA11yPageState {
    counter: i32,
    enabled: bool,
    focus_message: &'static str,
}

impl Default for FocusA11yPageState {
    fn default() -> Self {
        Self {
            counter: 0,
            enabled: false,
            focus_message: "Ready for keyboard focus.",
        }
    }
}

impl FocusA11yPageState {
    /// Returns the current demo counter.
    pub(crate) fn counter(self) -> i32 {
        self.counter
    }

    /// Returns the derived accessibility state used by the page renderer.
    pub(crate) fn demo_state(self) -> A11yDemoState {
        a11y_demo_state(self.counter, self.enabled)
    }

    /// Returns the current user-facing focus message.
    pub(crate) fn focus_message(self) -> &'static str {
        self.focus_message
    }

    /// Returns whether the demo switch is enabled.
    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }

    /// Updates the focus message and returns whether the state changed.
    pub(crate) fn set_focus_message(&mut self, message: &'static str) -> bool {
        if self.focus_message == message {
            return false;
        }

        self.focus_message = message;
        true
    }

    /// Increments the demo counter and returns whether the state changed.
    pub(crate) fn increment_counter(&mut self) -> bool {
        self.counter += 1;
        true
    }

    /// Decrements the demo counter and returns whether the state changed.
    pub(crate) fn decrement_counter(&mut self) -> bool {
        let next = (self.counter - 1).max(0);
        if self.counter == next {
            return false;
        }

        self.counter = next;
        true
    }

    /// Resets the demo counter and returns whether the state changed.
    pub(crate) fn reset_counter(&mut self) -> bool {
        if self.counter == 0 {
            return false;
        }

        self.counter = 0;
        true
    }

    /// Toggles the demo switch and returns whether the state changed.
    pub(crate) fn toggle_enabled(&mut self) -> bool {
        self.enabled = !self.enabled;
        true
    }
}

/// Accessibility state surfaced by the demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A11yDemoState {
    /// Counter value exposed by the spin button.
    pub counter: i32,
    /// Toggle state exposed by the switch.
    pub toggled: Toggled,
    /// Role used for the counter control.
    pub counter_role: Role,
    /// Role used for the toggle control.
    pub toggle_role: Role,
}

/// Builds the accessibility state summary from plain view state.
pub const fn a11y_demo_state(counter: i32, enabled: bool) -> A11yDemoState {
    A11yDemoState {
        counter,
        toggled: if enabled {
            Toggled::True
        } else {
            Toggled::False
        },
        counter_role: Role::SpinButton,
        toggle_role: Role::Switch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_a11y_page_state_tracks_message_counter_and_switch_state() {
        let mut state = FocusA11yPageState::default();

        assert_eq!(state.focus_message(), "Ready for keyboard focus.");
        assert_eq!(state.demo_state(), a11y_demo_state(0, false));
        assert!(!state.enabled());
        assert!(!state.decrement_counter());
        assert!(!state.reset_counter());

        assert!(state.increment_counter());
        assert_eq!(state.demo_state(), a11y_demo_state(1, false));
        assert!(state.toggle_enabled());
        assert_eq!(state.demo_state(), a11y_demo_state(1, true));
        assert!(state.set_focus_message("Focus moved."));
        assert_eq!(state.focus_message(), "Focus moved.");
        assert!(state.reset_counter());
        assert_eq!(state.demo_state(), a11y_demo_state(0, true));
    }
}
