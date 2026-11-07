//! Host-portable tests of the production Metal frame-state transitions.
//! This does not compile or execute the native display-link integration.

#![cfg(not(target_os = "macos"))]

#[path = "../src/display_link/metal_state.rs"]
mod metal_state;
