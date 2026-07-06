//! Official GPUI primitives for the Open GPUI component ecosystem.

pub mod field_state;
pub mod focus_ring;
pub mod roving_focus_group;
pub mod trigger_a11y;

pub use field_state::FieldState;
pub use focus_ring::{DEFAULT_FOCUS_RING_WIDTH, FocusRing};
pub use roving_focus_group::{
    active_index_from_str_keys, first_enabled, last_enabled, next_enabled, paged_navigation_target,
    roving_navigation_target, typeahead_target, vertical_roving_navigation_target,
};
pub use trigger_a11y::{
    UiA11yElementExt, gpui_accessible_action_from_ui, gpui_orientation_from_ui, gpui_role_from_ui,
    gpui_toggled_from_ui,
};
