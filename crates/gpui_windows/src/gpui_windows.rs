#![cfg(target_os = "windows")]

mod clipboard;
mod destination_list;
mod direct_manipulation;
mod direct_write;
mod directx_atlas;
mod directx_devices;
mod directx_renderer;
mod dispatcher;
mod display;
mod events;
mod keyboard;
#[cfg(any(test, feature = "test-support"))]
mod native_test_observation;
mod platform;
mod system_settings;
mod util;
mod vsync;
mod window;
mod wrapper;

pub(crate) use clipboard::*;
pub(crate) use destination_list::*;
pub(crate) use direct_write::*;
pub(crate) use directx_atlas::*;
pub(crate) use directx_devices::*;
pub(crate) use directx_renderer::*;
pub(crate) use dispatcher::*;
pub(crate) use display::*;
pub(crate) use events::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use system_settings::*;
pub(crate) use util::*;
pub(crate) use vsync::*;
pub(crate) use window::*;
pub(crate) use wrapper::*;

pub use platform::WindowsPlatform;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use native_test_observation::{
    NativeWindowTestCaptureOwner, NativeWindowTestEvent, NativeWindowTestEventKind,
    NativeWindowTestIdentity, NativeWindowTestMessage, NativeWindowTestMessageDisposition,
    NativeWindowTestObservation, NativeWindowTestObservationGuard, NativeWindowTestPoint,
    begin_native_window_test_observation,
};

pub(crate) use windows::Win32::Foundation::HWND;
