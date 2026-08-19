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
mod native_test_foreground;
#[cfg(any(test, feature = "test-support"))]
mod native_test_harness;
#[cfg(any(test, feature = "test-support"))]
mod native_test_observation;
#[cfg(any(test, feature = "test-support"))]
mod native_test_pointer;
#[cfg(any(test, feature = "test-support"))]
mod native_test_scenario;
#[cfg(any(test, feature = "test-support"))]
mod native_test_window;
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

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use native_test_scenario::native_test_confirm_scenario_behavior;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use native_test_harness::{
    NATIVE_TEST_INPUT_CANARY, NativeNoInputGenerationDriftGuard, NativeTestDisplay,
    NativeTestMixedDpiDisplayPair, NativeTestOpaqueWindow, NativeTestPointerAction,
    NativeTestProcessWindowCensus, NativeTestSystemPointerGuard, NativeTestWindowProbe,
    arm_native_no_input_generation_drift, native_test_acquire_foreground_window,
    native_test_client_screen_bounds, native_test_displays, native_test_inject_system_pointer,
    native_test_inject_system_pointer_sequence, native_test_logical_client_point_to_screen,
    native_test_mixed_dpi_display_pair, native_test_non_shell_root_window_at,
    native_test_process_window_census, native_test_raise_window,
    native_test_release_primary_button_best_effort, native_test_virtual_screen_bounds,
    native_test_window_is_above, native_test_window_probe, native_test_window_rect,
};

pub(crate) use windows::Win32::Foundation::HWND;
