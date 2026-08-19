#![cfg_attr(test, allow(dead_code, unused_imports))]

#[cfg(feature = "test-support")]
use crate::WindowsDisplay;
pub use crate::native_test_pointer::{
    NATIVE_TEST_INPUT_CANARY, NativeTestPointerAction, NativeTestSystemPointerGuard,
    native_test_inject_system_pointer, native_test_inject_system_pointer_sequence,
    native_test_release_primary_button_best_effort, native_test_virtual_screen_bounds,
};
#[allow(unused_imports)]
pub(crate) use crate::native_test_pointer::{
    native_test_inject_system_pointer_sequence_with_extra_info,
    native_test_inject_system_pointer_with_extra_info,
    native_test_release_primary_button_best_effort_with_extra_info,
};
pub use crate::native_test_window::{
    NativeTestOpaqueWindow, NativeTestWindowProbe, native_test_client_screen_bounds,
    native_test_logical_client_point_to_screen, native_test_non_shell_root_window_at,
    native_test_raise_window, native_test_window_is_above, native_test_window_probe,
    native_test_window_rect,
};
use anyhow::{Context as _, Result, ensure};
use open_gpui::{AnyWindowHandle, Bounds, DevicePixels, DisplayId, Point, WindowId, point};
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
            EnumWindows, FindWindowExW, GetWindowThreadProcessId, HWND_MESSAGE, IsWindow,
        },
    },
    core::BOOL,
};

const DRIFT_ARMED: u8 = 0;
const DRIFT_APPLIED: u8 = 1;
const DRIFT_TARGET_MISSING: u8 = 2;
const PROCESS_WINDOW_CENSUS_ATTEMPTS: usize = 8;
const MIXED_DPI_SCALE_EPSILON: f32 = 0.001;

/// One physical Windows display observation for native interactive tests.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeTestDisplay {
    display_id: DisplayId,
    physical_bounds: Bounds<DevicePixels>,
    physical_visible_bounds: Bounds<DevicePixels>,
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

    /// Returns the monitor work area in physical desktop coordinates.
    pub const fn physical_visible_bounds(self) -> Bounds<DevicePixels> {
        self.physical_visible_bounds
    }

    /// Returns the effective DPI scale sampled with the monitor rectangle.
    pub const fn scale_factor(self) -> f32 {
        self.scale_factor
    }
}

/// One deterministically selected distinct-DPI display pair for native interactive tests.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeTestMixedDpiDisplayPair {
    lower: NativeTestDisplay,
    higher: NativeTestDisplay,
    negative_display: NativeTestDisplay,
    negative_physical_point: Point<DevicePixels>,
}

impl NativeTestMixedDpiDisplayPair {
    /// Returns the lower-scale display in the selected physical pair.
    pub const fn lower(self) -> NativeTestDisplay {
        self.lower
    }

    /// Returns the higher-scale display in the selected physical pair.
    pub const fn higher(self) -> NativeTestDisplay {
        self.higher
    }

    /// Returns the selected pair member whose visible desktop area contains a negative point.
    pub const fn negative_display(self) -> NativeTestDisplay {
        self.negative_display
    }

    /// Returns a physical point inside the selected member's visible bounds with a negative axis.
    pub const fn negative_physical_point(self) -> Point<DevicePixels> {
        self.negative_physical_point
    }
}

/// Enumerates physical display facts from the owning Windows backend.
#[doc(hidden)]
#[cfg(feature = "test-support")]
pub fn native_test_displays() -> Vec<NativeTestDisplay> {
    WindowsDisplay::available_for_native_test()
        .into_iter()
        .map(|display| NativeTestDisplay {
            display_id: display.display_id,
            physical_bounds: display.physical_bounds(),
            physical_visible_bounds: display.physical_visible_bounds(),
            scale_factor: display.scale_factor(),
        })
        .collect()
}

/// Selects the maximum-scale-delta pair with a visible physical desktop point on a negative axis.
#[doc(hidden)]
#[cfg(feature = "test-support")]
pub fn native_test_mixed_dpi_display_pair() -> Result<NativeTestMixedDpiDisplayPair> {
    let displays = native_test_displays();
    select_native_test_mixed_dpi_display_pair(&displays).with_context(|| {
        format!("mixed-DPI display capability selection failed for snapshot {displays:?}")
    })
}

fn select_native_test_mixed_dpi_display_pair(
    displays: &[NativeTestDisplay],
) -> Result<NativeTestMixedDpiDisplayPair> {
    let mut displays = displays.to_vec();
    displays.sort_unstable_by_key(|display| u64::from(display.display_id()));
    let mut has_distinct_dpi = false;
    let mut selected = None::<(
        NativeTestDisplay,
        NativeTestDisplay,
        NativeTestDisplay,
        Point<DevicePixels>,
        f32,
    )>;

    for first_index in 0..displays.len() {
        for second_index in first_index + 1..displays.len() {
            let first = displays[first_index];
            let second = displays[second_index];
            let scale_delta = (first.scale_factor() - second.scale_factor()).abs();
            if scale_delta <= MIXED_DPI_SCALE_EPSILON {
                continue;
            }
            has_distinct_dpi = true;
            let first_negative_point = negative_visible_physical_point(first);
            let second_negative_point = negative_visible_physical_point(second);
            if first_negative_point.is_none() && second_negative_point.is_none()
                || selected.is_some_and(|(_, _, _, _, best_delta)| scale_delta <= best_delta)
            {
                continue;
            }
            let (lower, higher) = if first.scale_factor() < second.scale_factor() {
                (first, second)
            } else {
                (second, first)
            };
            let (negative_display, negative_physical_point) =
                if let Some(point) = negative_visible_physical_point(lower) {
                    (lower, point)
                } else if let Some(point) = negative_visible_physical_point(higher) {
                    (higher, point)
                } else {
                    continue;
                };
            selected = Some((
                lower,
                higher,
                negative_display,
                negative_physical_point,
                scale_delta,
            ));
        }
    }

    ensure!(
        has_distinct_dpi,
        "mixed-DPI native tests require two real displays with distinct effective DPI"
    );
    let (lower, higher, negative_display, negative_physical_point, _) = selected.context(
        "mixed-DPI native tests require a distinct-DPI display pair with a negative physical origin and a usable negative physical desktop point",
    )?;
    Ok(NativeTestMixedDpiDisplayPair {
        lower,
        higher,
        negative_display,
        negative_physical_point,
    })
}

fn negative_visible_physical_point(display: NativeTestDisplay) -> Option<Point<DevicePixels>> {
    let bounds = display.physical_visible_bounds();
    let left = i64::from(bounds.origin.x.0);
    let top = i64::from(bounds.origin.y.0);
    let right = left.checked_add(i64::from(bounds.size.width.0))?;
    let bottom = top.checked_add(i64::from(bounds.size.height.0))?;
    if right <= left || bottom <= top {
        return None;
    }

    if left < 0 {
        let negative_right = right.min(0);
        if left < negative_right {
            return Some(point(
                DevicePixels(midpoint_inclusive_range(left, negative_right)?),
                DevicePixels(midpoint_inclusive_range(top, bottom)?),
            ));
        }
    }
    if top < 0 {
        let negative_bottom = bottom.min(0);
        if top < negative_bottom {
            return Some(point(
                DevicePixels(midpoint_inclusive_range(left, right)?),
                DevicePixels(midpoint_inclusive_range(top, negative_bottom)?),
            ));
        }
    }
    None
}

fn midpoint_inclusive_range(start: i64, end_exclusive: i64) -> Option<i32> {
    let end_inclusive = end_exclusive.checked_sub(1)?;
    i32::try_from(start.checked_add(end_inclusive)?.checked_div(2)?).ok()
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

    fn display(id: u64, scale_factor: f32, origin_x: i32, origin_y: i32) -> NativeTestDisplay {
        let bounds = Bounds::new(
            point(DevicePixels(origin_x), DevicePixels(origin_y)),
            size(DevicePixels(1_920), DevicePixels(1_080)),
        );
        NativeTestDisplay {
            display_id: DisplayId::from(id),
            physical_bounds: bounds,
            physical_visible_bounds: bounds,
            scale_factor,
        }
    }

    fn display_with_visible_origin(
        id: u64,
        scale_factor: f32,
        physical_origin_x: i32,
        visible_origin_x: i32,
    ) -> NativeTestDisplay {
        NativeTestDisplay {
            display_id: DisplayId::from(id),
            physical_bounds: Bounds::new(
                point(DevicePixels(physical_origin_x), DevicePixels(0)),
                size(DevicePixels(1_920), DevicePixels(1_080)),
            ),
            physical_visible_bounds: Bounds::new(
                point(DevicePixels(visible_origin_x), DevicePixels(0)),
                size(DevicePixels(1_280), DevicePixels(1_024)),
            ),
            scale_factor,
        }
    }

    #[test]
    fn mixed_dpi_pair_prefers_max_scale_delta_with_negative_origin() {
        let displays = [
            display(10, 1.0, -1_920, 0),
            display(20, 1.25, 0, 0),
            display(30, 2.0, 1_920, 0),
        ];

        let pair = select_native_test_mixed_dpi_display_pair(&displays)
            .expect("the mixed-DPI capability pair should be available");

        assert_eq!(pair.lower().display_id(), DisplayId::from(10));
        assert_eq!(pair.higher().display_id(), DisplayId::from(30));
        assert_eq!(pair.negative_display().display_id(), DisplayId::from(10));
        let negative_point = pair.negative_physical_point();
        assert!(negative_point.x.0 < 0 || negative_point.y.0 < 0);
        assert!(
            pair.negative_display()
                .physical_visible_bounds()
                .contains(&negative_point)
        );
    }

    #[test]
    fn mixed_dpi_pair_requires_distinct_dpi() {
        let error = select_native_test_mixed_dpi_display_pair(&[
            display(10, 1.25, -1_920, 0),
            display(20, 1.25, 0, 0),
        ])
        .expect_err("equal-scale displays must fail closed");

        assert!(error.to_string().contains("distinct effective DPI"));
    }

    #[test]
    fn mixed_dpi_pair_requires_negative_physical_origin() {
        let error = select_native_test_mixed_dpi_display_pair(&[
            display(10, 1.0, 0, 0),
            display(20, 2.0, 1_920, 0),
        ])
        .expect_err("non-negative display topology must fail closed");

        assert!(error.to_string().contains("negative physical origin"));
    }

    #[test]
    fn mixed_dpi_pair_requires_a_negative_visible_physical_point() {
        let error = select_native_test_mixed_dpi_display_pair(&[
            display_with_visible_origin(10, 1.0, -64, 0),
            display(20, 2.0, 1_920, 0),
        ])
        .expect_err("a hidden negative strip must not satisfy negative routing capability");

        assert!(
            error
                .to_string()
                .contains("negative physical desktop point")
        );
    }

    #[test]
    fn mixed_dpi_pair_selection_is_stable_across_snapshot_order() {
        let first_order = [
            display(30, 1.0, -3_840, 0),
            display(40, 2.0, 0, 0),
            display(10, 1.0, -1_920, 0),
            display(20, 2.0, 1_920, 0),
        ];
        let second_order = [
            display(20, 2.0, 1_920, 0),
            display(10, 1.0, -1_920, 0),
            display(40, 2.0, 0, 0),
            display(30, 1.0, -3_840, 0),
        ];

        let first = select_native_test_mixed_dpi_display_pair(&first_order)
            .expect("the first display snapshot should select a mixed-DPI pair");
        let second = select_native_test_mixed_dpi_display_pair(&second_order)
            .expect("the reordered display snapshot should select a mixed-DPI pair");

        assert_eq!(first, second);
    }

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
