use crate::WindowsDisplay;
use anyhow::{Context as _, Result, ensure};
use open_gpui::{AnyWindowHandle, Bounds, DevicePixels, DisplayId, WindowId};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use windows::{
    Win32::{
        Foundation::{ERROR_INVALID_WINDOW_HANDLE, GetLastError, HWND, LPARAM},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, EnumWindows, FindWindowExW, GetWindowThreadProcessId,
            HWND_MESSAGE, HWND_TOP, IsWindow, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
    core::{BOOL, w},
};

const DRIFT_ARMED: u8 = 0;
const DRIFT_APPLIED: u8 = 1;
const DRIFT_TARGET_MISSING: u8 = 2;
const PROCESS_WINDOW_CENSUS_ATTEMPTS: usize = 8;

/// One physical Windows display observation for native interactive tests.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeTestDisplay {
    display_id: DisplayId,
    physical_bounds: Bounds<DevicePixels>,
    scale_factor: f32,
}

impl NativeTestDisplay {
    /// Returns the GPUI display identity backed by the sampled monitor.
    pub const fn display_id(self) -> DisplayId {
        self.display_id
    }

    /// Returns the monitor rectangle in physical desktop coordinates.
    pub const fn physical_bounds(self) -> Bounds<DevicePixels> {
        self.physical_bounds
    }

    /// Returns the effective DPI scale sampled with the monitor rectangle.
    pub const fn scale_factor(self) -> f32 {
        self.scale_factor
    }
}

/// Enumerates physical display facts from the owning Windows backend.
#[doc(hidden)]
pub fn native_test_displays() -> Vec<NativeTestDisplay> {
    WindowsDisplay::available_for_native_test()
        .into_iter()
        .map(|display| NativeTestDisplay {
            display_id: display.display_id,
            physical_bounds: display.physical_bounds(),
            scale_factor: display.scale_factor(),
        })
        .collect()
}

/// Makes one live HWND the deterministic foreground input target for a native test.
///
/// Windows can reject a bare `SetForegroundWindow` call when the test runner is not the current
/// foreground process. This adapter temporarily joins the relevant input queues, prepares the
/// desktop state, and always detaches before returning. The actual test should still prove its
/// input and capture transitions independently.
#[doc(hidden)]
pub fn native_test_acquire_foreground_window(native_handle: isize) -> Result<()> {
    crate::native_test_foreground::acquire_foreground_window(HWND(
        native_handle as *mut core::ffi::c_void,
    ))
}

/// A live process-owned HWND census collected before the process exits.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeTestProcessWindowCensus {
    top_level_hwnds: Vec<isize>,
    message_only_hwnds: Vec<isize>,
}

impl NativeTestProcessWindowCensus {
    /// Returns ordinary top-level HWNDs owned by the sampled process.
    pub fn top_level_hwnds(&self) -> &[isize] {
        &self.top_level_hwnds
    }

    /// Returns message-only HWNDs owned by the sampled process.
    pub fn message_only_hwnds(&self) -> &[isize] {
        &self.message_only_hwnds
    }
}

/// Enumerates ordinary and message-only HWNDs while the process is still alive.
#[doc(hidden)]
pub fn native_test_process_window_census(process_id: u32) -> Result<NativeTestProcessWindowCensus> {
    let mut previous = None;
    for _ in 0..PROCESS_WINDOW_CENSUS_ATTEMPTS {
        let current = NativeTestProcessWindowCensus {
            top_level_hwnds: native_test_windows_for_process(
                process_id,
                NativeTestWindowRoot::Desktop,
            )?,
            message_only_hwnds: native_test_windows_for_process(
                process_id,
                NativeTestWindowRoot::MessageOnly,
            )?,
        };
        if previous.as_ref() == Some(&current) {
            return Ok(current);
        }
        previous = Some(current);
        std::thread::yield_now();
    }
    Err(anyhow::anyhow!(
        "native HWND census did not stabilize across {PROCESS_WINDOW_CENSUS_ATTEMPTS} consecutive process samples"
    ))
}

#[derive(Clone, Copy)]
enum NativeTestWindowRoot {
    Desktop,
    MessageOnly,
}

fn native_test_windows_for_process(
    process_id: u32,
    root: NativeTestWindowRoot,
) -> Result<Vec<isize>> {
    match root {
        NativeTestWindowRoot::Desktop => native_test_desktop_windows_for_process(process_id),
        NativeTestWindowRoot::MessageOnly => {
            native_test_message_only_windows_for_process(process_id)
        }
    }
}

fn native_test_desktop_windows_for_process(process_id: u32) -> Result<Vec<isize>> {
    for attempt in 0..PROCESS_WINDOW_CENSUS_ATTEMPTS {
        let mut enumeration = NativeTestProcessWindowEnumeration {
            process_id,
            windows: Vec::new(),
            failed_window: None,
        };
        let callback_data =
            LPARAM(&mut enumeration as *mut NativeTestProcessWindowEnumeration as isize);
        let result = unsafe { EnumWindows(Some(collect_native_test_window), callback_data) };
        if let Some((window, error)) = enumeration.failed_window {
            return Err(anyhow::anyhow!(
                "native HWND census could not read process identity for window handle {window:#x}: {error:?}"
            ));
        }
        match result {
            Ok(()) => {
                enumeration.windows.sort_unstable();
                enumeration.windows.dedup();
                return Ok(enumeration.windows);
            }
            Err(error)
                if error.code() == ERROR_INVALID_WINDOW_HANDLE.to_hresult()
                    && attempt + 1 < PROCESS_WINDOW_CENSUS_ATTEMPTS =>
            {
                std::thread::yield_now();
            }
            Err(error) => {
                return Err(error).context(
                    "native HWND census enumeration failed before reaching the end of Z-order",
                );
            }
        }
    }
    Err(anyhow::anyhow!(
        "native desktop HWND census exhausted transient invalid-handle retries"
    ))
}

fn native_test_message_only_windows_for_process(process_id: u32) -> Result<Vec<isize>> {
    for attempt in 0..PROCESS_WINDOW_CENSUS_ATTEMPTS {
        match sample_native_test_message_only_windows(process_id) {
            Ok(windows) => return Ok(windows),
            Err(error)
                if error
                    .downcast_ref::<windows::core::Error>()
                    .is_some_and(|error| {
                        error.code() == ERROR_INVALID_WINDOW_HANDLE.to_hresult()
                    })
                    && attempt + 1 < PROCESS_WINDOW_CENSUS_ATTEMPTS =>
            {
                std::thread::yield_now();
            }
            Err(error) => return Err(error),
        }
    }
    Err(anyhow::anyhow!(
        "native message-only HWND census exhausted transient invalid-handle retries"
    ))
}

fn sample_native_test_message_only_windows(process_id: u32) -> Result<Vec<isize>> {
    let mut windows = Vec::new();
    let mut seen = HashSet::new();
    let mut previous = None;

    loop {
        let window = unsafe {
            FindWindowExW(
                Some(HWND_MESSAGE),
                previous,
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::null(),
            )
        };
        let Ok(window) = window else {
            break;
        };
        ensure!(
            seen.insert(window.0 as isize),
            "native message-only HWND enumeration repeated window handle {:#x}",
            window.0 as isize
        );

        let mut owner_process_id = 0;
        if unsafe { GetWindowThreadProcessId(window, Some(&mut owner_process_id)) } == 0 {
            let error = windows::core::Error::from_hresult(unsafe { GetLastError() }.to_hresult());
            if error.code() == ERROR_INVALID_WINDOW_HANDLE.to_hresult()
                || !unsafe { IsWindow(Some(window)).as_bool() }
            {
                return Err(error.into());
            }
            return Err(error).context(format!(
                "native message-only HWND census could not read process identity for window handle {:#x}",
                window.0 as isize
            ));
        }
        if owner_process_id == process_id {
            windows.push(window.0 as isize);
        }
        previous = Some(window);
    }

    windows.sort_unstable();
    windows.dedup();
    Ok(windows)
}

struct NativeTestProcessWindowEnumeration {
    process_id: u32,
    windows: Vec<isize>,
    failed_window: Option<(isize, windows::Win32::Foundation::WIN32_ERROR)>,
}

unsafe extern "system" fn collect_native_test_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let enumeration = unsafe { &mut *(data.0 as *mut NativeTestProcessWindowEnumeration) };
    let mut process_id = 0;
    if unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) } == 0 {
        let error = unsafe { GetLastError() };
        if error == ERROR_INVALID_WINDOW_HANDLE || !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return BOOL(1);
        }
        enumeration.failed_window = Some((hwnd.0 as isize, error));
        return BOOL(0);
    }
    if process_id == enumeration.process_id {
        enumeration.windows.push(hwnd.0 as isize);
    }
    BOOL(1)
}

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

    /// Returns the raw HWND value without transferring ownership.
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

#[derive(Clone)]
pub(crate) struct NativeNoInputGenerationDriftRequest {
    target: WindowId,
    outcome: Arc<AtomicU8>,
}

impl NativeNoInputGenerationDriftRequest {
    pub(crate) const fn target(&self) -> WindowId {
        self.target
    }

    pub(crate) fn mark_applied(&self) {
        self.outcome.store(DRIFT_APPLIED, Ordering::Release);
    }

    pub(crate) fn mark_target_missing(&self) {
        self.outcome.store(DRIFT_TARGET_MISSING, Ordering::Release);
    }
}

static NATIVE_NO_INPUT_GENERATION_DRIFT: Mutex<Option<NativeNoInputGenerationDriftRequest>> =
    Mutex::new(None);

/// Arms one deterministic generation change between the first and verifying hit-stack samples.
#[doc(hidden)]
pub fn arm_native_no_input_generation_drift(
    window: AnyWindowHandle,
) -> Result<NativeNoInputGenerationDriftGuard> {
    let target = window.window_id();
    let outcome = Arc::new(AtomicU8::new(DRIFT_ARMED));
    let request = NativeNoInputGenerationDriftRequest {
        target,
        outcome: Arc::clone(&outcome),
    };
    let mut pending = NATIVE_NO_INPUT_GENERATION_DRIFT.lock();
    ensure!(
        pending.is_none(),
        "another native no-input generation drift is already armed"
    );
    *pending = Some(request);
    Ok(NativeNoInputGenerationDriftGuard {
        target,
        outcome,
        finished: false,
    })
}

pub(crate) fn pending_native_no_input_generation_drift_target() -> Option<WindowId> {
    NATIVE_NO_INPUT_GENERATION_DRIFT
        .lock()
        .as_ref()
        .map(NativeNoInputGenerationDriftRequest::target)
}

pub(crate) fn take_native_no_input_generation_drift(
    target: WindowId,
) -> Option<NativeNoInputGenerationDriftRequest> {
    let mut pending = NATIVE_NO_INPUT_GENERATION_DRIFT.lock();
    if pending
        .as_ref()
        .is_some_and(|request| request.target == target)
    {
        pending.take()
    } else {
        None
    }
}

/// Proves that an armed native hit-stack drift was consumed by the owning platform.
#[doc(hidden)]
pub struct NativeNoInputGenerationDriftGuard {
    target: WindowId,
    outcome: Arc<AtomicU8>,
    finished: bool,
}

impl NativeNoInputGenerationDriftGuard {
    /// Completes the probe and fails if the platform did not consume the exact target hook.
    pub fn finish(mut self) -> Result<()> {
        self.clear_pending();
        self.finished = true;
        match self.outcome.load(Ordering::Acquire) {
            DRIFT_APPLIED => Ok(()),
            DRIFT_TARGET_MISSING => Err(anyhow::anyhow!(
                "the armed no-input window disappeared before generation drift was applied"
            )),
            _ => Err(anyhow::anyhow!(
                "the owning Windows hit-stack sampler did not consume the armed generation drift"
            )),
        }
    }

    fn clear_pending(&self) {
        let mut pending = NATIVE_NO_INPUT_GENERATION_DRIFT.lock();
        if pending.as_ref().is_some_and(|request| {
            request.target == self.target && Arc::ptr_eq(&request.outcome, &self.outcome)
        }) {
            pending.take();
        }
    }
}

impl Drop for NativeNoInputGenerationDriftGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.clear_pending();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, size};

    #[test]
    fn process_window_census_tracks_live_and_retired_test_window() {
        let window = NativeTestOpaqueWindow::create(Bounds {
            origin: point(DevicePixels(32), DevicePixels(32)),
            size: size(DevicePixels(96), DevicePixels(64)),
        })
        .expect("create native census test window");
        let handle = window.native_handle();

        let live = native_test_process_window_census(std::process::id())
            .expect("sample current-process HWND census while test window is live");
        assert!(live.top_level_hwnds().contains(&handle));

        drop(window);

        let retired = native_test_process_window_census(std::process::id())
            .expect("sample current-process HWND census after test window retirement");
        assert!(!retired.top_level_hwnds().contains(&handle));
    }
}
