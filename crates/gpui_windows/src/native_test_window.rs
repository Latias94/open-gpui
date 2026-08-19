#![cfg_attr(test, allow(dead_code))]

use anyhow::{Context as _, Result, ensure};
use open_gpui::{Bounds, DevicePixels, Pixels, Point, point};
use windows::{
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GW_HWNDNEXT, GetClassNameW, GetWindow, GetWindowRect,
            GetWindowThreadProcessId, HWND_TOP, IsWindow, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
    core::w,
};

/// Owns one ordinary opaque HWND used by point-scoped native tests.
#[doc(hidden)]
pub struct NativeTestOpaqueWindow {
    hwnd: HWND,
}

impl NativeTestOpaqueWindow {
    /// Creates a hidden opaque non-topmost test window at exact physical bounds.
    pub fn create_hidden(bounds: Bounds<DevicePixels>) -> Result<Self> {
        ensure!(
            bounds.size.width.0 > 0 && bounds.size.height.0 > 0,
            "native test opaque-window bounds must be non-empty"
        );
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("BUTTON"),
                w!("open-gpui native opaque test window"),
                WS_POPUP,
                bounds.origin.x.0,
                bounds.origin.y.0,
                bounds.size.width.0,
                bounds.size.height.0,
                None,
                None,
                None,
                None,
            )
        }
        .context("failed to create native opaque test HWND")?;
        Ok(Self { hwnd })
    }

    /// Creates and presents an opaque non-topmost test window at exact physical bounds.
    pub fn create(bounds: Bounds<DevicePixels>) -> Result<Self> {
        let window = Self::create_hidden(bounds)?;
        window.present()?;
        Ok(window)
    }

    /// Presents a prepared opaque test window without activating it or changing its geometry.
    pub fn present(&self) -> Result<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        }
        .context("failed to place native opaque test HWND")?;
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
        Ok(())
    }

    /// Returns the owned HWND without transferring ownership.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Returns the raw HWND value for process-census boundaries.
    pub fn native_handle(&self) -> isize {
        self.hwnd.0 as isize
    }
}

impl Drop for NativeTestOpaqueWindow {
    fn drop(&mut self) {
        if unsafe { IsWindow(Some(self.hwnd)).as_bool() } {
            let _ = unsafe { DestroyWindow(self.hwnd) };
        }
    }
}

/// Diagnostic metadata for the native root window covering one physical point.
#[doc(hidden)]
pub struct NativeTestWindowProbe {
    point: POINT,
    hwnd: HWND,
    class_name: String,
    process_id: u32,
    rect: Option<RECT>,
}

impl std::fmt::Debug for NativeTestWindowProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeTestWindowProbe")
            .field("point", &self.point)
            .field("hwnd", &self.hwnd)
            .field("class_name", &self.class_name)
            .field("process_id", &self.process_id)
            .field("rect", &self.rect)
            .finish()
    }
}

/// Raises one HWND in the normal Z-order band without activation or geometry mutation.
#[doc(hidden)]
pub fn native_test_raise_window(hwnd: HWND) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
    }
    .context("failed to raise native test window in the normal Z-order band")
}

/// Returns the non-shell native root window covering one physical desktop point.
#[doc(hidden)]
pub fn native_test_non_shell_root_window_at(physical_point: POINT) -> Option<HWND> {
    crate::platform::window_from_point_root(point(
        DevicePixels(physical_point.x),
        DevicePixels(physical_point.y),
    ))
}

/// Returns whether `upper` precedes `lower` in the current native Z-order.
#[doc(hidden)]
pub fn native_test_window_is_above(upper: HWND, lower: HWND) -> bool {
    if upper == HWND::default() || lower == HWND::default() || upper == lower {
        return false;
    }
    let mut current = upper;
    for _ in 0..4096 {
        let Ok(next) = (unsafe { GetWindow(current, GW_HWNDNEXT) }) else {
            return false;
        };
        if next == lower {
            return true;
        }
        current = next;
    }
    false
}

/// Returns diagnostic metadata for one point-scoped native root hit.
#[doc(hidden)]
pub fn native_test_window_probe(point: POINT, hwnd: HWND) -> NativeTestWindowProbe {
    let mut class_name = [0_u16; 128];
    let class_length = unsafe { GetClassNameW(hwnd, &mut class_name) }.max(0) as usize;
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    NativeTestWindowProbe {
        point,
        hwnd,
        class_name: String::from_utf16_lossy(&class_name[..class_length]),
        process_id,
        rect: native_test_window_rect(hwnd).ok(),
    }
}

/// Samples one native outer rectangle.
#[doc(hidden)]
pub fn native_test_window_rect(hwnd: HWND) -> Result<RECT> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }
        .context("the native Windows test harness could not sample an HWND rectangle")?;
    Ok(rect)
}

/// Samples one native client rectangle in physical desktop coordinates.
#[doc(hidden)]
pub fn native_test_client_screen_bounds(hwnd: HWND) -> Result<Bounds<DevicePixels>> {
    crate::native_physical_client_bounds(hwnd)
        .context("the native Windows test harness could not sample the client rectangle")
}

/// Converts one logical client point to physical desktop coordinates.
#[doc(hidden)]
pub fn native_test_logical_client_point_to_screen(
    hwnd: HWND,
    point: Point<Pixels>,
    scale_factor: f32,
) -> Result<POINT> {
    let mut point = POINT {
        x: (point.x.as_f32() * scale_factor).round() as i32,
        y: (point.y.as_f32() * scale_factor).round() as i32,
    };
    ensure!(
        unsafe { ClientToScreen(hwnd, &mut point).as_bool() },
        "the native Windows test harness could not convert a committed client point for HWND {hwnd:?}"
    );
    Ok(point)
}
