//! Host-portable tests of the production Metal frame-state transitions.
//! This does not compile or execute the native display-link integration.

#![cfg(not(any(target_os = "macos", target_os = "ios")))]

#[path = "../src/display_link/metal_state.rs"]
mod metal_state;
