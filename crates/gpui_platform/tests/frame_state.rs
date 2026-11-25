//! Host-portable tests of the production AppKit frame-state transitions.
//! This does not compile or execute the native display-link integration.

#![cfg(not(target_os = "macos"))]

#[path = "../src/macos/display_link/frame_state.rs"]
mod frame_state;
