#![cfg(any(target_os = "macos", target_os = "ios"))]
//! Shared Apple rendering, dispatch, text and system services for Open GPUI.
//!
//! Native platform backends retain application lifecycle, windowing and input ownership.

mod dispatcher;
pub mod display_link;
pub mod keychain;
mod metal_atlas;
pub mod metal_renderer;
pub mod process_info;

#[cfg(feature = "font-kit")]
mod open_type;
#[cfg(feature = "font-kit")]
mod text_system;

use objc2::MainThreadMarker;

pub use dispatcher::{AppleActivity, AppleDispatcher};
#[cfg(feature = "font-kit")]
pub use text_system::AppleTextSystem;

/// Returns the cached main-thread marker, or `None` on any other thread.
#[inline]
pub fn main_thread_marker() -> Option<MainThreadMarker> {
    thread_local! {
        static MAIN_THREAD_MARKER: Option<MainThreadMarker> = MainThreadMarker::new();
    }
    MAIN_THREAD_MARKER.with(|marker| *marker)
}
