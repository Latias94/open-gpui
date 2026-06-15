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
