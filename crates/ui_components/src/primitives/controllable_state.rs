//! GPUI-facing controllable state primitives.

pub use open_gpui_ui_core::ControllableState;

/// Resolves a generic controllable state using an optional controlled value or a default seed.
pub fn resolve<T>(
    controlled: Option<T>,
    default_value: impl FnOnce() -> T,
) -> ControllableState<T> {
    ControllableState::resolve(controlled, default_value)
}

/// Resolves an open-state primitive from an optional controlled override and a default seed.
pub fn open_state(
    controlled_open: Option<bool>,
    default_open: impl FnOnce() -> bool,
) -> ControllableState<bool> {
    resolve(controlled_open, default_open)
}
