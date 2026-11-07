#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use linux::current_platform;

#[cfg(all(test, not(any(target_os = "linux", target_os = "freebsd"))))]
#[path = "linux/wayland/frame_state.rs"]
mod wayland_frame_state;
