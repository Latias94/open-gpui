mod dispatcher;
mod display_connection;
mod headless;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod interaction_gate;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod power;
mod system_notifications;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub use dispatcher::*;
pub(crate) use display_connection::{Backend, DisplayConnection, select_backend};
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

use std::rc::Rc;

use open_gpui::WindowingModes;

/// Returns the default platform implementation for the current OS.
///
/// A windowed platform connects to the display server the process environment names, or starts
/// headless when it names none, and can switch between those modes later. A headless platform
/// stays headless.
pub fn current_platform(headless: bool) -> Rc<dyn open_gpui::Platform> {
    if headless || std::env::var_os("ZED_HEADLESS").is_some() {
        linux_platform(WindowingModes::HEADLESS)
    } else {
        linux_platform(WindowingModes::all())
    }
}

/// Returns a platform that may switch among `allowed_modes`.
///
/// It starts windowed in the process's own environment, or headless if that names no allowed
/// display server. Set another initial mode with [`open_gpui::Application::with_windowing`].
/// This factory does not adopt a descriptor named by `WAYLAND_SOCKET`; transfer an owned stream
/// through `linux_platform_with_startup` with the `wayland` feature when starting from an
/// inherited Wayland connection.
/// The Host's environment and descriptors remain unchanged.
pub fn linux_platform(allowed_modes: WindowingModes) -> Rc<dyn open_gpui::Platform> {
    Rc::new(LinuxPlatform::new(allowed_modes))
}

/// Creates a platform with explicitly owned Wayland startup resources.
///
/// The caller transfers an inherited compositor socket as an owned stream, after arranging its
/// exclusive ownership at process startup. The stream must not also be used by the Host or another
/// Wayland client. No process environment is modified. The socket and token are used only for the
/// initial connection; starting headless drops them. A supplied socket does not require
/// `WAYLAND_DISPLAY`, but Wayland must be allowed by `allowed_modes`.
///
/// Supply a reconnectable address with [`open_gpui::Application::with_windowing`] if child
/// commands or a later attach must reach the same compositor. An owned protocol connection
/// cannot supply that address or be reused after detach. GPUI removes `WAYLAND_SOCKET` and
/// the consumed activation token from its opener/restart commands. Hosts that spawn their own
/// children must arrange the same one-shot resource custody themselves;
/// [`open_gpui::GraphicalEnvironment::apply_to`] applies display variables only.
#[cfg(feature = "wayland")]
pub fn linux_platform_with_startup(
    allowed_modes: WindowingModes,
    wayland_socket: Option<std::os::unix::net::UnixStream>,
    activation_token: Option<String>,
) -> Rc<dyn open_gpui::Platform> {
    Rc::new(LinuxPlatform::new_with_startup(
        allowed_modes,
        StartupEnvironment::owned(wayland_socket, activation_token),
    ))
}
