#![cfg_attr(test, allow(dead_code))]

use anyhow::{Context as _, Result, ensure};
use open_gpui::{Bounds, DevicePixels, point, size};
use std::mem::size_of;
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Input::KeyboardAndMouse::{
            GetCapture, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_MOVE_NOCOALESCE,
            MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, ReleaseCapture, SendInput, SetCapture,
        },
        WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetSystemMetrics, IsWindow, SM_CXVIRTUALSCREEN,
            SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetCursorPos,
            SetForegroundWindow,
        },
    },
};

/// Extra-info marker attached to system pointer input emitted by this harness.
#[doc(hidden)]
pub const NATIVE_TEST_INPUT_CANARY: usize = 0x4f47_5044;

/// One system pointer action emitted through the owning Windows test harness.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTestPointerAction {
    Move,
    PrimaryDown,
    PrimaryUp,
}

impl NativeTestPointerAction {
    fn flags(self) -> windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS {
        match self {
            Self::Move => MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE,
            Self::PrimaryDown => MOUSEEVENTF_LEFTDOWN,
            Self::PrimaryUp => MOUSEEVENTF_LEFTUP,
        }
    }
}

/// Injects one system pointer action at an exact physical desktop point.
#[doc(hidden)]
pub fn native_test_inject_system_pointer(
    point: POINT,
    action: NativeTestPointerAction,
) -> Result<()> {
    native_test_inject_system_pointer_with_extra_info(point, action, NATIVE_TEST_INPUT_CANARY)
}

pub(crate) fn native_test_inject_system_pointer_with_extra_info(
    point: POINT,
    action: NativeTestPointerAction,
    extra_info: usize,
) -> Result<()> {
    let virtual_screen = native_test_virtual_screen_bounds()?;
    let input = native_test_pointer_input(virtual_screen, point, action, extra_info)?;
    native_test_send_system_pointer_inputs(&[input])
}

/// Injects one ordered system pointer sequence through SendInput.
#[doc(hidden)]
pub fn native_test_inject_system_pointer_sequence(
    events: &[(POINT, NativeTestPointerAction)],
) -> Result<()> {
    native_test_inject_system_pointer_sequence_with_extra_info(events, NATIVE_TEST_INPUT_CANARY)
}

pub(crate) fn native_test_inject_system_pointer_sequence_with_extra_info(
    events: &[(POINT, NativeTestPointerAction)],
    extra_info: usize,
) -> Result<()> {
    ensure!(
        !events.is_empty(),
        "native Windows tests require at least one system pointer event"
    );
    let virtual_screen = native_test_virtual_screen_bounds()?;
    let inputs = events
        .iter()
        .copied()
        .map(|(point, action)| native_test_pointer_input(virtual_screen, point, action, extra_info))
        .collect::<Result<Vec<_>>>()?;
    native_test_send_system_pointer_inputs(&inputs)
}

fn native_test_pointer_input(
    virtual_screen: Bounds<DevicePixels>,
    point: POINT,
    action: NativeTestPointerAction,
    extra_info: usize,
) -> Result<INPUT> {
    let (dx, dy) = absolute_system_pointer_coordinates(virtual_screen, point)?;
    Ok(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | action.flags(),
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    })
}

/// Best-effort terminal release used only from panic and timeout cleanup.
#[doc(hidden)]
pub fn native_test_release_primary_button_best_effort() -> bool {
    native_test_release_primary_button_best_effort_with_extra_info(NATIVE_TEST_INPUT_CANARY)
}

pub(crate) fn native_test_release_primary_button_best_effort_with_extra_info(
    extra_info: usize,
) -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTUP,
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };
    native_test_send_system_pointer_inputs(&[input]).is_ok()
}

fn native_test_send_system_pointer_inputs(inputs: &[INPUT]) -> Result<()> {
    ensure!(
        unsafe { SendInput(inputs, size_of::<INPUT>() as i32) } == inputs.len() as u32,
        "native Windows system input was rejected by the desktop or UIPI boundary"
    );
    Ok(())
}

/// Restores native pointer state after one interactive test scope.
#[doc(hidden)]
#[must_use = "dropping the guard immediately would restore the captured native pointer state"]
pub struct NativeTestSystemPointerGuard {
    original_position: POINT,
    original_foreground: HWND,
    original_capture_owner: HWND,
    input_extra_info: usize,
    primary_button_down: bool,
}

impl NativeTestSystemPointerGuard {
    /// Captures the current native pointer state for a public test-support scope.
    pub fn capture() -> Result<Self> {
        Self::capture_with_extra_info(NATIVE_TEST_INPUT_CANARY)
    }

    pub(crate) fn capture_with_extra_info(input_extra_info: usize) -> Result<Self> {
        let mut original_position = POINT::default();
        unsafe { GetCursorPos(&mut original_position) }
            .context("the native Windows test harness could not read the system cursor")?;
        Ok(Self {
            original_position,
            original_foreground: unsafe { GetForegroundWindow() },
            original_capture_owner: unsafe { GetCapture() },
            input_extra_info,
            primary_button_down: false,
        })
    }

    /// Injects one system pointer action while retaining cleanup authority.
    pub fn inject(&mut self, point: POINT, action: NativeTestPointerAction) -> Result<()> {
        if action == NativeTestPointerAction::PrimaryDown {
            self.primary_button_down = true;
        }
        native_test_inject_system_pointer_with_extra_info(point, action, self.input_extra_info)?;
        self.commit_pointer_action(action);
        Ok(())
    }

    /// Injects one ordered pointer sequence while retaining cleanup authority.
    pub fn inject_sequence(&mut self, events: &[(POINT, NativeTestPointerAction)]) -> Result<()> {
        if events
            .iter()
            .any(|(_, action)| *action == NativeTestPointerAction::PrimaryDown)
        {
            self.primary_button_down = true;
        }
        native_test_inject_system_pointer_sequence_with_extra_info(events, self.input_extra_info)?;
        for (_, action) in events {
            self.commit_pointer_action(*action);
        }
        Ok(())
    }

    /// Best-effort terminal release while retaining cleanup authority on failure.
    pub fn release_primary_button_best_effort(&mut self) -> bool {
        if native_test_release_primary_button_best_effort_with_extra_info(self.input_extra_info) {
            self.primary_button_down = false;
            true
        } else {
            false
        }
    }

    fn commit_pointer_action(&mut self, action: NativeTestPointerAction) {
        match action {
            NativeTestPointerAction::PrimaryDown => self.primary_button_down = true,
            NativeTestPointerAction::PrimaryUp => self.primary_button_down = false,
            NativeTestPointerAction::Move => {}
        }
    }

    fn restore_capture_owner(&self) {
        let current_capture_owner = unsafe { GetCapture() };
        if current_capture_owner == self.original_capture_owner {
            return;
        }
        if self.original_capture_owner != HWND::default()
            && unsafe { IsWindow(Some(self.original_capture_owner)).as_bool() }
        {
            unsafe {
                SetCapture(self.original_capture_owner);
            }
        } else if current_capture_owner != HWND::default() {
            let _ = unsafe { ReleaseCapture() };
        }
    }
}

impl Drop for NativeTestSystemPointerGuard {
    fn drop(&mut self) {
        if self.primary_button_down {
            let _ = native_test_release_primary_button_best_effort_with_extra_info(
                self.input_extra_info,
            );
        }
        self.restore_capture_owner();
        let _ = unsafe { SetCursorPos(self.original_position.x, self.original_position.y) };
        if self.original_foreground != HWND::default()
            && unsafe { IsWindow(Some(self.original_foreground)).as_bool() }
        {
            let _ = unsafe { SetForegroundWindow(self.original_foreground) };
        }
    }
}

/// Returns the physical bounds of the current Win32 virtual desktop.
#[doc(hidden)]
pub fn native_test_virtual_screen_bounds() -> Result<Bounds<DevicePixels>> {
    let bounds = Bounds::new(
        point(
            DevicePixels(unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }),
            DevicePixels(unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }),
        ),
        size(
            DevicePixels(unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }),
            DevicePixels(unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }),
        ),
    );
    ensure!(
        bounds.size.width.0 > 1 && bounds.size.height.0 > 1,
        "native Windows tests require a non-empty virtual desktop: {bounds:?}"
    );
    Ok(bounds)
}

fn absolute_system_pointer_coordinates(
    virtual_screen: Bounds<DevicePixels>,
    physical_point: POINT,
) -> Result<(i32, i32)> {
    ensure!(
        virtual_screen.contains(&point(
            DevicePixels(physical_point.x),
            DevicePixels(physical_point.y),
        )),
        "native Windows injection point is outside the virtual desktop: point={physical_point:?}, bounds={virtual_screen:?}"
    );
    Ok((
        absolute_input_coordinate(
            physical_point.x,
            virtual_screen.origin.x.0,
            virtual_screen.size.width.0,
        ),
        absolute_input_coordinate(
            physical_point.y,
            virtual_screen.origin.y.0,
            virtual_screen.size.height.0,
        ),
    ))
}

fn absolute_input_coordinate(value: i32, origin: i32, extent: i32) -> i32 {
    debug_assert!(extent > 1);
    let numerator = (i64::from(value) - i64::from(origin)) * 65_535;
    (numerator / i64::from(extent - 1))
        .try_into()
        .expect("virtual-desktop coordinate must fit Win32 absolute input")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::{
        Win32::UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, IsWindow, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
            },
        },
        core::w,
    };

    struct NativePointerRestoreTestWindow {
        hwnd: HWND,
    }

    impl NativePointerRestoreTestWindow {
        fn create(left: i32, top: i32) -> Result<Self> {
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    None,
                    WS_POPUP | WS_VISIBLE,
                    left,
                    top,
                    64,
                    64,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .context("failed to create native pointer-restore test HWND")?;
            Ok(Self { hwnd })
        }
    }

    impl Drop for NativePointerRestoreTestWindow {
        fn drop(&mut self) {
            if unsafe { IsWindow(Some(self.hwnd)).as_bool() } {
                let _ = unsafe { DestroyWindow(self.hwnd) };
            }
        }
    }

    fn primary_button_is_down() -> bool {
        (unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) }) as u16 & 0x8000 != 0
    }

    fn cursor_is_at(expected: POINT) -> bool {
        let mut actual = POINT::default();
        unsafe { GetCursorPos(&mut actual) }.is_ok()
            && actual.x.abs_diff(expected.x) <= 1
            && actual.y.abs_diff(expected.y) <= 1
    }

    fn wait_until(description: &str, mut condition: impl FnMut() -> bool) -> Result<()> {
        let timeout = Duration::from_secs(2);
        let deadline = Instant::now() + timeout;
        while !condition() {
            ensure!(
                Instant::now() < deadline,
                "{description} did not converge within {timeout:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    #[test]
    fn absolute_system_input_coordinates_cover_negative_virtual_desktop_endpoints() {
        assert_eq!(absolute_input_coordinate(-1_920, -1_920, 3_840), 0);
        assert_eq!(absolute_input_coordinate(1_919, -1_920, 3_840), 65_535);
        assert_eq!(absolute_input_coordinate(-1, -1_920, 3_840), 32_758);
    }

    #[test]
    #[ignore = "requires an isolated interactive desktop with global pointer authority"]
    fn system_pointer_guard_drop_restores_owned_native_state() -> Result<()> {
        ensure!(
            std::env::var("OPEN_GPUI_NATIVE_INTERACTIVE")
                .ok()
                .is_some_and(|value| value == "1"),
            "native pointer restoration requires OPEN_GPUI_NATIVE_INTERACTIVE=1"
        );
        ensure!(
            unsafe { GetCapture() } == HWND::default(),
            "native pointer restoration requires a clean capture owner"
        );
        if primary_button_is_down() {
            let _ = native_test_release_primary_button_best_effort();
            anyhow::bail!(
                "native pointer restoration started with the primary button pressed; cleanup was attempted before rejecting the contaminated runner"
            );
        }

        let ambient_state = NativeTestSystemPointerGuard::capture()?;
        let virtual_screen = native_test_virtual_screen_bounds()?;
        let center = virtual_screen.center();
        let baseline_point = POINT {
            x: center.x.0.saturating_sub(48),
            y: center.y.0,
        };
        let mutated_point = POINT {
            x: center.x.0.saturating_add(48),
            y: center.y.0,
        };
        ensure!(
            virtual_screen.contains(&point(
                DevicePixels(baseline_point.x),
                DevicePixels(baseline_point.y),
            )) && virtual_screen.contains(&point(
                DevicePixels(mutated_point.x),
                DevicePixels(mutated_point.y),
            )),
            "native pointer restoration requires at least 96 physical pixels around the virtual-desktop center"
        );

        let baseline_window = NativePointerRestoreTestWindow::create(
            baseline_point.x.saturating_sub(32),
            baseline_point.y.saturating_sub(32),
        )?;
        let mutated_window = NativePointerRestoreTestWindow::create(
            mutated_point.x.saturating_sub(32),
            mutated_point.y.saturating_sub(32),
        )?;

        crate::native_test_foreground::acquire_foreground_window(baseline_window.hwnd)?;
        unsafe {
            SetCapture(baseline_window.hwnd);
        }
        unsafe { SetCursorPos(baseline_point.x, baseline_point.y) }
            .context("failed to establish the native pointer restoration baseline")?;
        wait_until("native pointer restoration baseline", || unsafe {
            GetForegroundWindow() == baseline_window.hwnd
                && GetCapture() == baseline_window.hwnd
                && cursor_is_at(baseline_point)
                && !primary_button_is_down()
        })?;

        let mut guard = NativeTestSystemPointerGuard::capture()?;
        crate::native_test_foreground::acquire_foreground_window(mutated_window.hwnd)?;
        unsafe {
            SetCapture(mutated_window.hwnd);
        }
        guard.inject(mutated_point, NativeTestPointerAction::Move)?;
        guard.inject(mutated_point, NativeTestPointerAction::PrimaryDown)?;
        wait_until("mutated native pointer state", || unsafe {
            GetForegroundWindow() == mutated_window.hwnd
                && GetCapture() == mutated_window.hwnd
                && cursor_is_at(mutated_point)
                && primary_button_is_down()
        })?;

        drop(guard);
        wait_until("restored native pointer state", || unsafe {
            GetForegroundWindow() == baseline_window.hwnd
                && GetCapture() == baseline_window.hwnd
                && cursor_is_at(baseline_point)
                && !primary_button_is_down()
        })?;

        drop(mutated_window);
        drop(baseline_window);
        drop(ambient_state);
        Ok(())
    }
}
