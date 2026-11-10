#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use linux::{current_platform, linux_platform};

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "wayland"))]
pub use linux::linux_platform_with_startup;

#[cfg(all(test, not(any(target_os = "linux", target_os = "freebsd"))))]
#[path = "linux/wayland/frame_state.rs"]
mod wayland_frame_state;

#[cfg(all(test, not(any(target_os = "linux", target_os = "freebsd"))))]
#[path = "linux/power/profile.rs"]
mod power_profile;
