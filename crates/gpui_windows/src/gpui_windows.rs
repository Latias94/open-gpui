#[cfg(any(target_os = "windows", test))]
mod color_glyph_bitmap;

#[cfg(target_os = "windows")]
mod clipboard;
#[cfg(target_os = "windows")]
mod destination_list;
#[cfg(target_os = "windows")]
mod dialog;
#[cfg(target_os = "windows")]
mod direct_manipulation;
#[cfg(target_os = "windows")]
mod direct_write;
#[cfg(target_os = "windows")]
mod directx_atlas;
#[cfg(target_os = "windows")]
mod directx_devices;
#[cfg(target_os = "windows")]
mod directx_renderer;
#[cfg(target_os = "windows")]
mod dispatcher;
#[cfg(target_os = "windows")]
mod display;
#[cfg(target_os = "windows")]
mod events;
#[cfg(target_os = "windows")]
mod keyboard;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_foreground;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_harness;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_observation;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_pointer;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_scenario;
#[cfg(all(target_os = "windows", any(test, feature = "test-support")))]
mod native_test_window;
#[cfg(target_os = "windows")]
mod platform;
#[cfg(target_os = "windows")]
mod renderer;
#[cfg(target_os = "windows")]
mod system_notifications;
#[cfg(target_os = "windows")]
mod system_settings;
#[cfg(target_os = "windows")]
mod util;
#[cfg(target_os = "windows")]
mod vsync;
#[cfg(target_os = "windows")]
mod window;
#[cfg(target_os = "windows")]
mod wrapper;

#[cfg(target_os = "windows")]
pub(crate) use clipboard::*;
#[cfg(target_os = "windows")]
pub(crate) use destination_list::*;
#[cfg(target_os = "windows")]
pub(crate) use direct_write::*;
#[cfg(target_os = "windows")]
pub(crate) use directx_atlas::*;
#[cfg(target_os = "windows")]
pub(crate) use directx_devices::*;
#[cfg(target_os = "windows")]
pub(crate) use directx_renderer::*;
#[cfg(target_os = "windows")]
pub(crate) use dispatcher::*;
#[cfg(target_os = "windows")]
pub(crate) use display::*;
#[cfg(target_os = "windows")]
pub(crate) use events::*;
#[cfg(target_os = "windows")]
pub(crate) use keyboard::*;
#[cfg(target_os = "windows")]
pub(crate) use platform::*;
#[cfg(target_os = "windows")]
pub(crate) use renderer::{WindowsRendererSource, WindowsWindowRenderer};
#[cfg(target_os = "windows")]
pub(crate) use system_notifications::*;
#[cfg(target_os = "windows")]
pub(crate) use system_settings::*;
#[cfg(target_os = "windows")]
pub(crate) use util::*;
#[cfg(target_os = "windows")]
pub(crate) use vsync::*;
#[cfg(target_os = "windows")]
pub(crate) use window::*;
#[cfg(target_os = "windows")]
pub(crate) use wrapper::*;

#[cfg(target_os = "windows")]
pub use platform::WindowsPlatform;
#[cfg(target_os = "windows")]
pub use renderer::{WindowsRenderer, WindowsRendererFactory, WindowsRendererWindow};

#[cfg(all(target_os = "windows", feature = "test-support"))]
#[doc(hidden)]
pub use native_test_observation::{
    NativeWindowTestCaptureOwner, NativeWindowTestEvent, NativeWindowTestEventKind,
    NativeWindowTestIdentity, NativeWindowTestMessage, NativeWindowTestMessageDisposition,
    NativeWindowTestObservation, NativeWindowTestObservationCursor,
    NativeWindowTestObservationGuard, NativeWindowTestPoint, begin_native_window_test_observation,
};

#[cfg(all(target_os = "windows", feature = "test-support"))]
#[doc(hidden)]
pub use native_test_scenario::native_test_confirm_scenario_behavior;

#[cfg(all(target_os = "windows", feature = "test-support"))]
#[doc(hidden)]
pub use native_test_harness::{
    NATIVE_TEST_INPUT_CANARY, NativeNoInputGenerationDriftGuard, NativeTestDisplay,
    NativeTestMixedDpiDisplayPair, NativeTestOpaqueWindow, NativeTestPointerAction,
    NativeTestProcessWindowCensus, NativeTestSystemPointerGuard, NativeTestWindowProbe,
    arm_native_no_input_generation_drift, native_test_acquire_foreground_window,
    native_test_client_screen_bounds, native_test_displays, native_test_highest_refresh_display,
    native_test_inject_system_pointer, native_test_inject_system_pointer_sequence,
    native_test_logical_client_point_to_screen, native_test_mixed_dpi_display_pair,
    native_test_non_shell_root_window_at, native_test_process_window_census,
    native_test_raise_window, native_test_release_primary_button_best_effort,
    native_test_virtual_screen_bounds, native_test_window_is_above, native_test_window_probe,
    native_test_window_rect,
};

#[cfg(target_os = "windows")]
pub(crate) use windows::Win32::Foundation::HWND;
