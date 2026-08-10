use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::atomic::Ordering,
};

use ::open_gpui_util::ResultExt;
use anyhow::Context as _;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::SystemServices::*,
        UI::{
            Controls::*,
            HiDpi::*,
            Input::{Ime::*, KeyboardAndMouse::*},
            WindowsAndMessaging::*,
        },
    },
    core::PCWSTR,
};

use crate::*;
use open_gpui::*;

pub(crate) const WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD: u32 = WM_USER + 3;
pub(crate) const WM_GPUI_DOCK_MENU_ACTION: u32 = WM_USER + 4;
pub(crate) const WM_GPUI_FORCE_UPDATE_WINDOW: u32 = WM_USER + 5;
pub(crate) const WM_GPUI_KEYBOARD_LAYOUT_CHANGED: u32 = WM_USER + 6;
pub(crate) const WM_GPUI_GPU_DEVICE_LOST: u32 = WM_USER + 7;
pub(crate) const WM_GPUI_KEYDOWN: u32 = WM_USER + 8;

const SIZE_MOVE_LOOP_TIMER_ID: usize = 1;

fn mouse_button_mask(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 1 << 0,
        MouseButton::Right => 1 << 1,
        MouseButton::Middle => 1 << 2,
        MouseButton::Navigate(NavigationDirection::Back) => 1 << 3,
        MouseButton::Navigate(NavigationDirection::Forward) => 1 << 4,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WindowsPointerCaptureState {
    pressed_buttons: u8,
    owns_native_capture: bool,
}

/// Monotonic identity for Windows pointer sessions on one HWND.
///
/// GPUI snapshots this epoch when it prepares a deferred framework release. The first native
/// attempt and every retry must remain bound to that snapshot so they cannot release an
/// intervening client or non-client pointer session.
#[derive(Default)]
pub(crate) struct WindowsNativePointerCaptureReleaseState {
    pointer_session_epoch: Cell<u64>,
}

impl WindowsNativePointerCaptureReleaseState {
    fn record_pointer_session_start(&self) {
        self.pointer_session_epoch.set(
            self.pointer_session_epoch
                .get()
                .checked_add(1)
                .expect("Windows pointer session epoch overflowed"),
        );
    }

    pub(crate) fn current_pointer_session_epoch(&self) -> u64 {
        self.pointer_session_epoch.get()
    }

    pub(crate) fn matches_pointer_session(&self, expected_epoch: u64) -> bool {
        self.current_pointer_session_epoch() == expected_epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsPointerCaptureInput {
    ButtonDown {
        button: MouseButton,
        acquire_native_capture: bool,
    },
    ButtonUp(MouseButton),
    Cancel(PointerCancelReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsPointerCaptureEffect {
    None,
    Acquire,
    Release,
    Cancel(PointerCancelReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsClientMouseButtonMessage {
    Down(MouseButton),
    Up(MouseButton),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowsCaptionButtonAction {
    Minimize,
    ToggleMaximize,
    Close,
}

impl WindowsCaptionButtonAction {
    fn from_hit_test(hit_test: u32) -> Option<Self> {
        match hit_test {
            HTMINBUTTON => Some(Self::Minimize),
            HTMAXBUTTON => Some(Self::ToggleMaximize),
            HTCLOSE => Some(Self::Close),
            _ => None,
        }
    }
}

impl WindowsClientMouseButtonMessage {
    fn button(self) -> MouseButton {
        match self {
            Self::Down(button) | Self::Up(button) => button,
        }
    }

    fn capture_input(self, acquire_native_capture: bool) -> WindowsPointerCaptureInput {
        match self {
            Self::Down(button) => WindowsPointerCaptureInput::ButtonDown {
                button,
                acquire_native_capture,
            },
            Self::Up(button) => WindowsPointerCaptureInput::ButtonUp(button),
        }
    }

    fn platform_input(
        self,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
    ) -> PlatformInput {
        match self {
            Self::Down(button) => PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count,
                first_mouse: false,
            }),
            Self::Up(button) => PlatformInput::MouseUp(MouseUpEvent {
                button,
                position,
                modifiers,
                click_count,
            }),
        }
    }
}

fn decode_client_mouse_button_message(
    message: u32,
    wparam: WPARAM,
) -> Option<WindowsClientMouseButtonMessage> {
    let xbutton = || match wparam.hiword() {
        XBUTTON1 => Some(MouseButton::Navigate(NavigationDirection::Back)),
        XBUTTON2 => Some(MouseButton::Navigate(NavigationDirection::Forward)),
        _ => None,
    };

    match message {
        WM_LBUTTONDOWN => Some(WindowsClientMouseButtonMessage::Down(MouseButton::Left)),
        WM_RBUTTONDOWN => Some(WindowsClientMouseButtonMessage::Down(MouseButton::Right)),
        WM_MBUTTONDOWN => Some(WindowsClientMouseButtonMessage::Down(MouseButton::Middle)),
        WM_XBUTTONDOWN => xbutton().map(WindowsClientMouseButtonMessage::Down),
        WM_LBUTTONUP => Some(WindowsClientMouseButtonMessage::Up(MouseButton::Left)),
        WM_RBUTTONUP => Some(WindowsClientMouseButtonMessage::Up(MouseButton::Right)),
        WM_MBUTTONUP => Some(WindowsClientMouseButtonMessage::Up(MouseButton::Middle)),
        WM_XBUTTONUP => xbutton().map(WindowsClientMouseButtonMessage::Up),
        _ => None,
    }
}

fn may_own_pointer_session(message: u32, wparam: WPARAM) -> bool {
    matches!(
        message,
        WM_MOUSEMOVE
            | WM_CAPTURECHANGED
            | WM_CANCELMODE
            | WM_NCMOUSEMOVE
            | WM_NCLBUTTONDBLCLK
            | WM_NCLBUTTONDOWN
            | WM_NCRBUTTONDOWN
            | WM_NCMBUTTONDOWN
            | WM_NCLBUTTONUP
            | WM_NCRBUTTONUP
            | WM_NCMBUTTONUP
    ) || decode_client_mouse_button_message(message, wparam).is_some()
}

impl WindowsPointerCaptureState {
    fn has_active_session(self) -> bool {
        self.pressed_buttons != 0 || self.owns_native_capture
    }

    fn is_button_pressed(self, button: MouseButton) -> bool {
        self.pressed_buttons & mouse_button_mask(button) != 0
    }

    fn pressed_button(self) -> Option<MouseButton> {
        [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Navigate(NavigationDirection::Back),
            MouseButton::Navigate(NavigationDirection::Forward),
        ]
        .into_iter()
        .find(|button| self.is_button_pressed(*button))
    }

    fn transition(self, input: WindowsPointerCaptureInput) -> (Self, WindowsPointerCaptureEffect) {
        match input {
            WindowsPointerCaptureInput::ButtonDown {
                button,
                acquire_native_capture,
            } => {
                let pressed_buttons = self.pressed_buttons | mouse_button_mask(button);
                let acquire_native_capture = acquire_native_capture && !self.owns_native_capture;
                let effect = if acquire_native_capture {
                    WindowsPointerCaptureEffect::Acquire
                } else {
                    WindowsPointerCaptureEffect::None
                };
                (
                    Self {
                        pressed_buttons,
                        owns_native_capture: self.owns_native_capture || acquire_native_capture,
                    },
                    effect,
                )
            }
            WindowsPointerCaptureInput::ButtonUp(button) => {
                let pressed_buttons = self.pressed_buttons & !mouse_button_mask(button);
                let session_ended = self.pressed_buttons != 0 && pressed_buttons == 0;
                let effect = if session_ended && self.owns_native_capture {
                    WindowsPointerCaptureEffect::Release
                } else {
                    WindowsPointerCaptureEffect::None
                };
                (
                    Self {
                        pressed_buttons,
                        owns_native_capture: self.owns_native_capture && !session_ended,
                    },
                    effect,
                )
            }
            WindowsPointerCaptureInput::Cancel(reason) if self.pressed_buttons != 0 => {
                (Self::default(), WindowsPointerCaptureEffect::Cancel(reason))
            }
            WindowsPointerCaptureInput::Cancel(_) => {
                (Self::default(), WindowsPointerCaptureEffect::None)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WindowsInputDispatchState {
    #[default]
    Idle,
    AcquiringNativeCapture {
        pending_terminal_cancel: Option<PointerCancelReason>,
    },
    Dispatching {
        pending_terminal_cancel: Option<PointerCancelReason>,
        terminal_cancel_reserved: bool,
    },
    RecoveringAfterPanic {
        pending_terminal_cancel: Option<PointerCancelReason>,
        terminal_cancel_reserved: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowsInputPanicRecovery {
    pending_terminal_cancel: Option<PointerCancelReason>,
    terminal_cancel_reserved: bool,
}

impl WindowsInputDispatchState {
    fn defer_terminal_cancel(self, reason: PointerCancelReason) -> Self {
        match self {
            Self::AcquiringNativeCapture {
                pending_terminal_cancel: None,
            } => Self::AcquiringNativeCapture {
                pending_terminal_cancel: Some(reason),
            },
            Self::AcquiringNativeCapture {
                pending_terminal_cancel: Some(_),
            } => self,
            Self::Dispatching {
                pending_terminal_cancel: None,
                terminal_cancel_reserved,
            } => Self::Dispatching {
                pending_terminal_cancel: Some(reason),
                terminal_cancel_reserved,
            },
            Self::Dispatching {
                pending_terminal_cancel: Some(_),
                ..
            } => self,
            state => state,
        }
    }

    fn reserve_terminal_cancel(self) -> Self {
        match self {
            Self::Dispatching {
                pending_terminal_cancel,
                ..
            } => Self::Dispatching {
                pending_terminal_cancel,
                terminal_cancel_reserved: true,
            },
            state => state,
        }
    }

    fn into_panic_recovery(self) -> Self {
        match self {
            Self::Dispatching {
                pending_terminal_cancel,
                terminal_cancel_reserved,
            } => Self::RecoveringAfterPanic {
                pending_terminal_cancel,
                terminal_cancel_reserved,
            },
            state => state,
        }
    }

    fn take_pending_terminal_cancel(self) -> (Self, Option<PointerCancelReason>) {
        match self {
            Self::Dispatching {
                pending_terminal_cancel: Some(reason),
                terminal_cancel_reserved,
            } => (
                Self::Dispatching {
                    pending_terminal_cancel: None,
                    terminal_cancel_reserved,
                },
                Some(reason),
            ),
            state => (state, None),
        }
    }

    fn take_panic_recovery(self) -> (Self, Option<WindowsInputPanicRecovery>) {
        match self {
            Self::RecoveringAfterPanic {
                pending_terminal_cancel,
                terminal_cancel_reserved,
            } => (
                Self::Idle,
                Some(WindowsInputPanicRecovery {
                    pending_terminal_cancel,
                    terminal_cancel_reserved,
                }),
            ),
            state => (state, None),
        }
    }
}

struct WindowsNativePointerCaptureAcquisitionGuard<'a> {
    state: &'a Cell<WindowsInputDispatchState>,
    active: bool,
}

impl<'a> WindowsNativePointerCaptureAcquisitionGuard<'a> {
    fn begin(state: &'a Cell<WindowsInputDispatchState>) -> Self {
        assert_eq!(
            state.get(),
            WindowsInputDispatchState::Idle,
            "native pointer capture acquisition re-entered active input dispatch"
        );
        state.set(WindowsInputDispatchState::AcquiringNativeCapture {
            pending_terminal_cancel: None,
        });
        Self {
            state,
            active: true,
        }
    }

    fn finish(mut self) -> Option<PointerCancelReason> {
        let pending_terminal_cancel = match self.state.get() {
            WindowsInputDispatchState::AcquiringNativeCapture {
                pending_terminal_cancel,
            } => pending_terminal_cancel,
            state => panic!(
                "native pointer capture acquisition left an unexpected dispatch state: {state:?}"
            ),
        };
        self.state.set(WindowsInputDispatchState::Idle);
        self.active = false;
        pending_terminal_cancel
    }
}

impl Drop for WindowsNativePointerCaptureAcquisitionGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.state.set(WindowsInputDispatchState::Idle);
        }
    }
}

fn should_reserve_pointer_cancel_after_callback_panic(
    pointer_session_active: bool,
    panic_recovery: Option<WindowsInputPanicRecovery>,
) -> bool {
    matches!(
        panic_recovery,
        Some(WindowsInputPanicRecovery {
            pending_terminal_cancel,
            terminal_cancel_reserved: false,
        }) if pointer_session_active || pending_terminal_cancel.is_some()
    )
}

struct WindowsInputDispatchGuard<'a> {
    state: &'a Cell<WindowsInputDispatchState>,
    active: bool,
}

impl<'a> WindowsInputDispatchGuard<'a> {
    fn begin(
        state: &'a Cell<WindowsInputDispatchState>,
        pending_terminal_cancel: Option<PointerCancelReason>,
    ) -> Self {
        assert_eq!(
            state.get(),
            WindowsInputDispatchState::Idle,
            "Windows input re-entered before the active callback finalized"
        );
        state.set(WindowsInputDispatchState::Dispatching {
            pending_terminal_cancel,
            terminal_cancel_reserved: false,
        });
        Self {
            state,
            active: true,
        }
    }

    fn finish(mut self) -> Option<PointerCancelReason> {
        let (_, pending_terminal_cancel) = self.state.get().take_pending_terminal_cancel();
        self.state.set(WindowsInputDispatchState::Idle);
        self.active = false;
        pending_terminal_cancel
    }
}

impl Drop for WindowsInputDispatchGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let dispatch_state = self.state.get();
            if std::thread::panicking() {
                self.state.set(dispatch_state.into_panic_recovery());
            } else {
                self.state.set(WindowsInputDispatchState::Idle);
            }
        }
    }
}

fn dispatch_windows_input(
    callbacks: &Callbacks,
    dispatch_state: &Cell<WindowsInputDispatchState>,
    input: PlatformInput,
) -> DispatchEventResult {
    dispatch_windows_input_with_pending_cancel(callbacks, dispatch_state, input, None)
}

fn dispatch_windows_input_with_pending_cancel(
    callbacks: &Callbacks,
    dispatch_state: &Cell<WindowsInputDispatchState>,
    input: PlatformInput,
    pending_terminal_cancel: Option<PointerCancelReason>,
) -> DispatchEventResult {
    let dispatch_guard = WindowsInputDispatchGuard::begin(dispatch_state, pending_terminal_cancel);
    let result = callbacks.input.dispatch(input);
    // Capture notifications can re-enter while the callback is owned here. Deliver their
    // terminal event only after the outer core dispatch returns.
    let pending_terminal_cancel = dispatch_guard.finish();
    if let Some(reason) = pending_terminal_cancel {
        dispatch_windows_input(
            callbacks,
            dispatch_state,
            PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
        );
    }

    result
}

fn dispatch_windows_pointer_cancel(
    callbacks: &Callbacks,
    dispatch_state: &Cell<WindowsInputDispatchState>,
    reason: PointerCancelReason,
) {
    let current_dispatch_state = dispatch_state.get();
    match current_dispatch_state {
        WindowsInputDispatchState::Idle => {
            dispatch_windows_input(
                callbacks,
                dispatch_state,
                PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
            );
        }
        WindowsInputDispatchState::AcquiringNativeCapture { .. } => {
            dispatch_state.set(current_dispatch_state.defer_terminal_cancel(reason));
        }
        WindowsInputDispatchState::Dispatching {
            pending_terminal_cancel: None,
            terminal_cancel_reserved: false,
        } => match callbacks.input.reserve_reentrant_pointer_cancel(reason) {
            NativePointerCancelReservation::Reserved => {
                dispatch_state.set(current_dispatch_state.reserve_terminal_cancel());
            }
            NativePointerCancelReservation::UnleasedTestFallback => {
                dispatch_state.set(current_dispatch_state.defer_terminal_cancel(reason));
            }
            outcome => {
                log::error!("failed to reserve reentrant pointer cancellation: {outcome:?}");
            }
        },
        WindowsInputDispatchState::Dispatching { .. } => {}
        WindowsInputDispatchState::RecoveringAfterPanic { .. } => {
            log::error!(
                "ignored pointer cancellation while the preceding input callback is recovering"
            );
        }
    }
}

struct WindowsClientPointerBoundary<'a> {
    pointer_capture: &'a Cell<WindowsPointerCaptureState>,
    native_pointer_capture_release: &'a WindowsNativePointerCaptureReleaseState,
    pressed_caption_button: &'a Cell<Option<WindowsCaptionButtonAction>>,
    callbacks: &'a Callbacks,
    dispatch_state: &'a Cell<WindowsInputDispatchState>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WindowsClientButtonOutcome {
    dispatch_result: Option<isize>,
    capture_acquisition_failed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WindowsNonClientButtonOutcome {
    consumed: bool,
    caption_action: Option<WindowsCaptionButtonAction>,
    capture_acquisition_failed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WindowsPointerCancelOutcome {
    canceled: bool,
    release_native_capture: bool,
}

struct WindowsNonClientPointerBoundary<'a> {
    pointer: WindowsClientPointerBoundary<'a>,
}

impl WindowsClientPointerBoundary<'_> {
    fn clear_pointer_capture(&self, reason: PointerCancelReason) -> WindowsPointerCancelOutcome {
        self.pressed_caption_button.take();
        let release_native_capture = self.pointer_capture.get().owns_native_capture;
        let (next_capture, effect) = self
            .pointer_capture
            .get()
            .transition(WindowsPointerCaptureInput::Cancel(reason));
        self.pointer_capture.set(next_capture);
        WindowsPointerCancelOutcome {
            canceled: matches!(effect, WindowsPointerCaptureEffect::Cancel(_)),
            release_native_capture,
        }
    }

    fn handle_button_message(
        &self,
        message: WindowsClientMouseButtonMessage,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
        apply_capture_effect: impl FnMut(WindowsPointerCaptureEffect) -> bool,
    ) -> WindowsClientButtonOutcome {
        if message == WindowsClientMouseButtonMessage::Up(MouseButton::Left) {
            self.pressed_caption_button.take();
        }
        self.handle_button_message_with_capture_policy(
            message,
            position,
            modifiers,
            click_count,
            true,
            apply_capture_effect,
        )
    }

    fn handle_non_client_button_message(
        &self,
        message: WindowsClientMouseButtonMessage,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
        apply_capture_effect: impl FnMut(WindowsPointerCaptureEffect) -> bool,
    ) -> WindowsClientButtonOutcome {
        self.handle_button_message_with_capture_policy(
            message,
            position,
            modifiers,
            click_count,
            false,
            apply_capture_effect,
        )
    }

    fn handle_button_message_with_capture_policy(
        &self,
        message: WindowsClientMouseButtonMessage,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
        acquire_native_capture: bool,
        mut apply_capture_effect: impl FnMut(WindowsPointerCaptureEffect) -> bool,
    ) -> WindowsClientButtonOutcome {
        let current_capture = self.pointer_capture.get();
        if matches!(message, WindowsClientMouseButtonMessage::Down(_))
            && !current_capture.has_active_session()
        {
            self.native_pointer_capture_release
                .record_pointer_session_start();
        }
        let (next_capture, capture_effect) =
            current_capture.transition(message.capture_input(acquire_native_capture));
        self.pointer_capture.set(next_capture);
        let pending_acquisition_cancel = if capture_effect == WindowsPointerCaptureEffect::Acquire {
            let acquisition =
                WindowsNativePointerCaptureAcquisitionGuard::begin(self.dispatch_state);
            let acquired = apply_capture_effect(capture_effect);
            let pending_cancel = acquisition.finish();
            (!acquired || pending_cancel.is_some() || self.pointer_capture.get() != next_capture)
                .then(|| pending_cancel.unwrap_or(PointerCancelReason::CaptureRevoked))
        } else {
            None
        };
        let capture_acquisition_failed = pending_acquisition_cancel.is_some();

        let dispatch = catch_unwind(AssertUnwindSafe(|| {
            dispatch_windows_input_with_pending_cancel(
                self.callbacks,
                self.dispatch_state,
                message.platform_input(position, modifiers, click_count),
                pending_acquisition_cancel,
            )
        }));

        if capture_effect == WindowsPointerCaptureEffect::Release {
            let _ = apply_capture_effect(capture_effect);
        }
        let dispatch_result = match dispatch {
            Ok(result) if result.propagate => Some(1),
            Ok(_) => Some(0),
            Err(payload) => std::panic::resume_unwind(payload),
        };
        WindowsClientButtonOutcome {
            dispatch_result,
            capture_acquisition_failed,
        }
    }

    fn handle_pointer_cancel(&self, reason: PointerCancelReason) -> WindowsPointerCancelOutcome {
        if matches!(
            self.dispatch_state.get(),
            WindowsInputDispatchState::AcquiringNativeCapture { .. }
        ) {
            self.pressed_caption_button.take();
            let pointer_capture = self.pointer_capture.get();
            let canceled = pointer_capture.pressed_buttons != 0;
            if canceled {
                dispatch_windows_pointer_cancel(self.callbacks, self.dispatch_state, reason);
            }
            return WindowsPointerCancelOutcome {
                canceled,
                release_native_capture: pointer_capture.owns_native_capture,
            };
        }

        let outcome = self.clear_pointer_capture(reason);
        if !outcome.canceled {
            return outcome;
        }
        dispatch_windows_pointer_cancel(self.callbacks, self.dispatch_state, reason);
        outcome
    }
}

impl WindowsNonClientPointerBoundary<'_> {
    fn handle_button_message(
        &self,
        message: WindowsClientMouseButtonMessage,
        hit_test: u32,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
        apply_capture_effect: impl FnMut(WindowsPointerCaptureEffect) -> bool,
    ) -> WindowsNonClientButtonOutcome {
        if message == WindowsClientMouseButtonMessage::Down(MouseButton::Left) {
            self.pointer.pressed_caption_button.take();
        }
        let pointer_outcome = self.pointer.handle_non_client_button_message(
            message,
            position,
            modifiers,
            click_count,
            apply_capture_effect,
        );
        let consumed = pointer_outcome.dispatch_result == Some(0);
        if consumed {
            if message == WindowsClientMouseButtonMessage::Up(MouseButton::Left) {
                self.pointer.pressed_caption_button.take();
            }
            return WindowsNonClientButtonOutcome {
                consumed: true,
                caption_action: None,
                capture_acquisition_failed: pointer_outcome.capture_acquisition_failed,
            };
        }

        match message {
            WindowsClientMouseButtonMessage::Down(MouseButton::Left) => {
                let caption_action = WindowsCaptionButtonAction::from_hit_test(hit_test);
                if caption_action.is_some()
                    && self
                        .pointer
                        .pointer_capture
                        .get()
                        .is_button_pressed(MouseButton::Left)
                {
                    self.pointer.pressed_caption_button.set(caption_action);
                }
                WindowsNonClientButtonOutcome {
                    consumed: caption_action.is_some(),
                    caption_action: None,
                    capture_acquisition_failed: pointer_outcome.capture_acquisition_failed,
                }
            }
            WindowsClientMouseButtonMessage::Up(MouseButton::Left) => {
                let released_caption_button = WindowsCaptionButtonAction::from_hit_test(hit_test);
                let caption_action = self
                    .pointer
                    .pressed_caption_button
                    .take()
                    .filter(|pressed| Some(*pressed) == released_caption_button);
                WindowsNonClientButtonOutcome {
                    consumed: caption_action.is_some(),
                    caption_action,
                    capture_acquisition_failed: pointer_outcome.capture_acquisition_failed,
                }
            }
            WindowsClientMouseButtonMessage::Down(_) | WindowsClientMouseButtonMessage::Up(_) => {
                WindowsNonClientButtonOutcome::default()
            }
        }
    }

    fn handle_pointer_cancel(&self, reason: PointerCancelReason) -> WindowsPointerCancelOutcome {
        self.pointer.handle_pointer_cancel(reason)
    }

    fn clear_pointer_capture(&self, reason: PointerCancelReason) -> WindowsPointerCancelOutcome {
        self.pointer.clear_pointer_capture(reason)
    }
}

impl WindowsWindowInner {
    pub(crate) fn dispatch_input(&self, input: PlatformInput) -> DispatchEventResult {
        if !matches!(&input, PlatformInput::PointerCanceled(_))
            && !self.provisional_accepts_interaction()
        {
            return DispatchEventResult {
                propagate: false,
                default_prevented: true,
            };
        }
        dispatch_windows_input(&self.state.callbacks, &self.state.input_dispatch, input)
    }

    fn client_pointer_boundary(&self) -> WindowsClientPointerBoundary<'_> {
        WindowsClientPointerBoundary {
            pointer_capture: &self.state.pointer_capture,
            native_pointer_capture_release: &self.state.native_pointer_capture_release,
            pressed_caption_button: &self.state.pressed_caption_button,
            callbacks: &self.state.callbacks,
            dispatch_state: &self.state.input_dispatch,
        }
    }

    fn non_client_pointer_boundary(&self) -> WindowsNonClientPointerBoundary<'_> {
        WindowsNonClientPointerBoundary {
            pointer: self.client_pointer_boundary(),
        }
    }

    fn apply_native_pointer_capture_effect(
        &self,
        handle: HWND,
        effect: WindowsPointerCaptureEffect,
    ) -> bool {
        // Native capture calls can synchronously re-enter through WM_CAPTURECHANGED. That
        // terminal callback must not inherit the physical frame of the outer pointer message.
        let _physical_frame_mask = self.mask_native_pointer_physical_frame_scope();
        match effect {
            WindowsPointerCaptureEffect::Acquire => unsafe {
                SetCapture(handle);
                #[cfg(test)]
                if let Some(replacement) = self
                    .state
                    .replace_next_pointer_capture_acquisition_with
                    .take()
                {
                    SetCapture(replacement);
                }
                let acquired = GetCapture() == handle;
                if !acquired {
                    log::error!("SetCapture did not grant pointer capture to the requesting HWND");
                }
                acquired
            },
            WindowsPointerCaptureEffect::Release if unsafe { GetCapture() } == handle => {
                match unsafe { ReleaseCapture() } {
                    Ok(()) => true,
                    Err(error) => {
                        log::error!("ReleaseCapture failed: {error}");
                        false
                    }
                }
            }
            WindowsPointerCaptureEffect::None
            | WindowsPointerCaptureEffect::Release
            | WindowsPointerCaptureEffect::Cancel(_) => true,
        }
    }

    fn settle_failed_native_pointer_capture_acquisition(&self, handle: HWND) {
        self.invalidate_native_pointer_physical_frame_scopes();
        let _physical_frame_mask = self.mask_native_pointer_physical_frame_scope();
        // The acquisition boundary already delivered the terminal cancellation after MouseDown.
        // Only backend bookkeeping and any surviving native capture remain to be retired here.
        let outcome = self
            .non_client_pointer_boundary()
            .clear_pointer_capture(PointerCancelReason::CaptureRevoked);
        if outcome.release_native_capture && unsafe { GetCapture() } == handle {
            unsafe { ReleaseCapture().log_err() };
        }
    }

    pub(crate) fn release_native_pointer_capture_after_framework_cancel(
        &self,
        _release_generation: u64,
        expected_pointer_session_epoch: u64,
    ) -> PlatformPointerCaptureReleaseOutcome {
        if self.is_native_window_terminal() {
            return PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal;
        }

        if !self
            .state
            .native_pointer_capture_release
            .matches_pointer_session(expected_pointer_session_epoch)
        {
            // A rejected release retried after a later MouseDown must never tear down the
            // replacement session. The original capture authority has been superseded.
            return PlatformPointerCaptureReleaseOutcome::Released;
        }

        #[cfg(test)]
        self.state
            .pointer_capture_release_history
            .borrow_mut()
            .push(_release_generation);

        self.invalidate_native_pointer_physical_frame_scopes();
        let _physical_frame_mask = self.mask_native_pointer_physical_frame_scope();
        self.state.pressed_caption_button.take();
        self.state
            .pointer_capture
            .set(WindowsPointerCaptureState::default());
        // ReleaseCapture can synchronously send WM_CAPTURECHANGED. Local state is terminal before
        // entering Win32, so the nested notification is cleanup-only and cannot emit a duplicate.
        if unsafe { GetCapture() } == self.hwnd {
            if let Err(error) = unsafe { ReleaseCapture() } {
                log::error!("ReleaseCapture failed: {error}");
            }
        }

        let outcome = if self.is_native_window_terminal() {
            PlatformPointerCaptureReleaseOutcome::NativeWindowTerminal
        } else if unsafe { GetCapture() } == self.hwnd {
            PlatformPointerCaptureReleaseOutcome::Rejected
        } else {
            PlatformPointerCaptureReleaseOutcome::Released
        };
        outcome
    }

    pub(crate) fn settle_pointer_capture_before_native_teardown(&self) {
        self.state.pressed_caption_button.take();
        let outcome = self
            .client_pointer_boundary()
            .clear_pointer_capture(PointerCancelReason::WindowClosed);
        if outcome.canceled
            && matches!(
                self.state.input_dispatch.get(),
                WindowsInputDispatchState::Dispatching { .. }
            )
        {
            dispatch_windows_pointer_cancel(
                &self.state.callbacks,
                &self.state.input_dispatch,
                PointerCancelReason::WindowClosed,
            );
        }
        if unsafe { GetCapture() } == self.hwnd {
            unsafe { ReleaseCapture().log_err() };
        }
    }

    pub(crate) fn settle_pointer_input_after_callback_panic(&self, msg: u32, wparam: WPARAM) {
        let (input_dispatch, panic_recovery) =
            self.state.input_dispatch.get().take_panic_recovery();
        self.state.input_dispatch.set(input_dispatch);

        if !may_own_pointer_session(msg, wparam) {
            return;
        }

        let owns_native_capture = unsafe { GetCapture() } == self.hwnd;
        let pointer_session_active =
            self.state.pointer_capture.get().has_active_session() || owns_native_capture;
        self.invalidate_native_pointer_physical_frame_scopes();
        self.state.pressed_caption_button.take();
        self.state
            .pointer_capture
            .set(WindowsPointerCaptureState::default());
        // ReleaseCapture can synchronously send WM_CAPTURECHANGED. The local state is already
        // terminal, so the nested message is cleanup-only. More importantly, no later
        // reservation failure or panic can leave the OS capture attached to this HWND.
        if owns_native_capture {
            unsafe { ReleaseCapture().log_err() };
        }
        if !should_reserve_pointer_cancel_after_callback_panic(
            pointer_session_active,
            panic_recovery,
        ) {
            return;
        }
        let reason = panic_recovery
            .and_then(|recovery| recovery.pending_terminal_cancel)
            .unwrap_or(PointerCancelReason::CaptureRevoked);
        #[cfg(test)]
        if self
            .state
            .panic_next_pointer_cancel_reservation
            .replace(false)
        {
            panic!("injected pointer-cancel reservation panic");
        }
        let reservation = self
            .state
            .callbacks
            .input
            .reserve_pointer_cancel_after_callback_panic(reason);

        match reservation {
            NativePointerCancelReservation::Reserved
            | NativePointerCancelReservation::ApplicationGone
            | NativePointerCancelReservation::IngressClosed
            | NativePointerCancelReservation::RetiredSlot => {}
            NativePointerCancelReservation::UnleasedTestFallback => {
                let cancellation = catch_unwind(AssertUnwindSafe(|| {
                    dispatch_windows_input(
                        &self.state.callbacks,
                        &self.state.input_dispatch,
                        PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
                    );
                }));
                if cancellation.is_err() {
                    log::error!(
                        "test-only pointer-cancel recovery panicked after an input callback panic"
                    );
                }
            }
            outcome => {
                log::error!("failed to reserve pointer panic cancellation: {outcome:?}");
            }
        }
    }

    pub(crate) fn handle_msg(
        self: &Rc<Self>,
        handle: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if self.provisional_requires_hit_transparency() {
            let blocked = match msg {
                WM_NCHITTEST => Some(HTTRANSPARENT as isize),
                WM_MOUSEACTIVATE => Some(MA_NOACTIVATEANDEAT as isize),
                WM_MOUSEMOVE
                | WM_NCMOUSEMOVE
                | WM_NCLBUTTONDBLCLK
                | WM_NCLBUTTONDOWN
                | WM_NCRBUTTONDOWN
                | WM_NCMBUTTONDOWN
                | WM_NCLBUTTONUP
                | WM_NCRBUTTONUP
                | WM_NCMBUTTONUP
                | WM_MOUSEWHEEL
                | WM_MOUSEHWHEEL
                | WM_SYSKEYUP
                | WM_KEYUP
                | WM_GPUI_KEYDOWN
                | WM_CHAR
                | WM_IME_STARTCOMPOSITION
                | WM_IME_COMPOSITION
                | WM_IME_ENDCOMPOSITION
                | DM_POINTERHITTEST
                | WM_GETOBJECT => Some(0),
                WM_SETCURSOR => Some(1),
                _ if decode_client_mouse_button_message(msg, wparam).is_some() => Some(0),
                _ => None,
            };
            if let Some(result) = blocked {
                return LRESULT(result);
            }
        }
        let handled = match msg {
            WM_MOUSEACTIVATE => {
                let policy = self.state.activation_policy();
                if !policy.focus_on_click {
                    Some(MA_NOACTIVATE as isize)
                } else {
                    // Eager activation keeps `active_window` coherent during the click dispatch.
                    unsafe { SetActiveWindow(handle).ok() };
                    None
                }
            }
            WM_ACTIVATE => self.handle_activate_msg(handle, wparam),
            WM_CREATE => self.handle_create_msg(handle),
            WM_MOVE => self.handle_move_msg(handle, lparam),
            WM_SIZE => self.handle_size_msg(wparam, lparam),
            WM_WINDOWPOSCHANGED => {
                self.state.advance_native_placement_epoch();
                None
            }
            WM_GETMINMAXINFO => self.handle_get_min_max_info_msg(lparam),
            WM_ENTERSIZEMOVE | WM_ENTERMENULOOP => self.handle_size_move_loop(handle),
            WM_EXITSIZEMOVE | WM_EXITMENULOOP => self.handle_size_move_loop_exit(handle),
            WM_TIMER => self.handle_timer_msg(handle, wparam),
            WM_NCCALCSIZE => self.handle_calc_client_size(handle, wparam, lparam),
            WM_DPICHANGED => self.handle_dpi_changed_msg(handle, wparam, lparam),
            WM_DISPLAYCHANGE => self.handle_display_change_msg(handle),
            WM_NCHITTEST => self.handle_hit_test_msg(handle, lparam),
            WM_PAINT => self.handle_paint_msg(handle),
            WM_CLOSE => self.handle_close_msg(),
            WM_DESTROY => self.handle_destroy_msg(),
            WM_MOUSEMOVE => self.handle_mouse_move_msg(handle, lparam, wparam),
            WM_MOUSELEAVE | WM_NCMOUSELEAVE => self.handle_mouse_leave_msg(),
            WM_CAPTURECHANGED => self.handle_pointer_capture_lost_msg(),
            WM_CANCELMODE => self.handle_cancel_mode_msg(handle),
            WM_NCMOUSEMOVE => self.handle_nc_mouse_move_msg(handle, lparam),
            // Treat double click as a second single click, since we track the double clicks ourselves.
            // If you don't interact with any elements, this will fall through to the windows default
            // behavior of toggling whether the window is maximized.
            WM_NCLBUTTONDBLCLK | WM_NCLBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Left, wparam, lparam)
            }
            WM_NCRBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Right, wparam, lparam)
            }
            WM_NCMBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Middle, wparam, lparam)
            }
            WM_NCLBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Left, wparam, lparam)
            }
            WM_NCRBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Right, wparam, lparam)
            }
            WM_NCMBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Middle, wparam, lparam)
            }
            _ if let Some(message) = decode_client_mouse_button_message(msg, wparam) => {
                self.handle_client_mouse_button_msg(handle, message, lparam)
            }
            WM_MOUSEWHEEL => self.handle_mouse_wheel_msg(handle, wparam, lparam),
            WM_MOUSEHWHEEL => self.handle_mouse_horizontal_wheel_msg(handle, wparam, lparam),
            WM_SYSKEYUP => self.handle_syskeyup_msg(wparam, lparam),
            WM_KEYUP => self.handle_keyup_msg(wparam, lparam),
            WM_GPUI_KEYDOWN => self.handle_keydown_msg(wparam, lparam),
            WM_CHAR => self.handle_char_msg(wparam),
            WM_IME_STARTCOMPOSITION => self.handle_ime_position(handle),
            WM_IME_COMPOSITION => self.handle_ime_composition(handle, lparam),
            WM_SETCURSOR => self.handle_set_cursor(handle, lparam),
            WM_SETTINGCHANGE => self.handle_system_settings_changed(handle, wparam, lparam),
            WM_INPUTLANGCHANGE => self.handle_input_language_changed(),
            WM_SHOWWINDOW => self.handle_window_visibility_changed(handle, wparam),
            WM_GPUI_FORCE_UPDATE_WINDOW if self.accepts_generation_bound_message(wparam.0) => {
                self.draw_window(handle, true)
            }
            WM_GPUI_GPU_DEVICE_LOST if self.accepts_generation_bound_message(wparam.0) => {
                self.handle_device_lost()
            }
            WM_GPUI_FORCE_UPDATE_WINDOW | WM_GPUI_GPU_DEVICE_LOST => Some(0),
            DM_POINTERHITTEST => self.handle_dm_pointer_hit_test(wparam),
            WM_GETOBJECT => self.handle_wm_getobject(wparam, lparam),
            _ => None,
        };
        if let Some(n) = handled {
            LRESULT(n)
        } else {
            unsafe { DefWindowProcW(handle, msg, wparam, lparam) }
        }
    }

    fn handle_move_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let origin = logical_point(
            lparam.signed_loword() as f32,
            lparam.signed_hiword() as f32,
            self.state.scale_factor.get(),
        );
        self.state.origin.set(origin);
        let size = self.state.logical_size.get();
        let center_x = origin.x.as_f32() + size.width.as_f32() / 2.;
        let center_y = origin.y.as_f32() + size.height.as_f32() / 2.;
        let monitor_bounds = self.state.display.get().bounds();
        if center_x < monitor_bounds.left().as_f32()
            || center_x > monitor_bounds.right().as_f32()
            || center_y < monitor_bounds.top().as_f32()
            || center_y > monitor_bounds.bottom().as_f32()
        {
            // center of the window may have moved to another monitor
            let monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONULL) };
            // minimize the window can trigger this event too, in this case,
            // monitor is invalid, we do nothing.
            if !monitor.is_invalid() && self.state.display.get().handle != monitor {
                // we will get the same monitor if we only have one
                self.state.display.set(WindowsDisplay::new(
                    WindowsDisplay::display_id_for_monitor(monitor),
                )?);
            }
        }
        let _ = with_windows_callback(&self.state.callbacks.moved, |callback| callback());
        Some(0)
    }

    fn handle_get_min_max_info_msg(&self, lparam: LPARAM) -> Option<isize> {
        let min_size = self.state.min_size?;
        let scale_factor = self.state.scale_factor.get();
        let boarder_offset = &self.state.border_offset;

        unsafe {
            let minmax_info = &mut *(lparam.0 as *mut MINMAXINFO);
            minmax_info.ptMinTrackSize.x = min_size.width.scale(scale_factor).as_f32() as i32
                + boarder_offset.width_offset.get();
            minmax_info.ptMinTrackSize.y = min_size.height.scale(scale_factor).as_f32() as i32
                + boarder_offset.height_offset.get();
        }
        Some(0)
    }

    fn handle_size_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // Don't resize the renderer when the window is minimized, but record that it was minimized so
        // that on restore the swap chain can be recreated via `update_drawable_size_even_if_unchanged`.
        if wparam.0 == SIZE_MINIMIZED as usize {
            let saved_request_frame = self.state.restore_from_minimized.take();
            if let Some(saved_request_frame) = saved_request_frame {
                self.state
                    .restore_from_minimized
                    .set(Some(saved_request_frame));
            } else {
                self.state
                    .restore_from_minimized
                    .set(self.state.callbacks.request_frame.take());
            }
            let _ = with_windows_callback(&self.state.callbacks.window_state_change, |callback| {
                callback()
            });
            return Some(0);
        }

        let width = lparam.loword().max(1) as i32;
        let height = lparam.hiword().max(1) as i32;
        let new_size = size(DevicePixels(width), DevicePixels(height));

        let scale_factor = self.state.scale_factor.get();
        let mut should_resize_renderer = false;
        if let Some(restore_from_minimized) = self.state.restore_from_minimized.take() {
            self.state
                .callbacks
                .request_frame
                .set(Some(restore_from_minimized));
        } else {
            should_resize_renderer = true;
        }

        self.handle_size_change(new_size, scale_factor, should_resize_renderer);
        Some(0)
    }

    fn handle_size_change(
        &self,
        device_size: Size<DevicePixels>,
        scale_factor: f32,
        should_resize_renderer: bool,
    ) {
        let new_logical_size = device_size.to_pixels(scale_factor);

        self.state.logical_size.set(new_logical_size);
        if should_resize_renderer
            && let Err(e) = self.state.renderer.borrow_mut().resize(device_size)
        {
            log::error!("Failed to resize renderer, invalidating devices: {}", e);
            self.state
                .invalidate_devices
                .store(true, std::sync::atomic::Ordering::Release);
        }
        let _ = with_windows_callback(&self.state.callbacks.resize, |callback| {
            callback(new_logical_size, scale_factor)
        });
    }

    fn handle_size_move_loop(&self, handle: HWND) -> Option<isize> {
        unsafe {
            let ret = SetTimer(
                Some(handle),
                SIZE_MOVE_LOOP_TIMER_ID,
                USER_TIMER_MINIMUM,
                None,
            );
            if ret == 0 {
                log::error!(
                    "unable to create timer: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
        None
    }

    fn handle_size_move_loop_exit(&self, handle: HWND) -> Option<isize> {
        unsafe {
            KillTimer(Some(handle), SIZE_MOVE_LOOP_TIMER_ID).log_err();
        }
        None
    }

    fn handle_timer_msg(&self, handle: HWND, wparam: WPARAM) -> Option<isize> {
        if wparam.0 == SIZE_MOVE_LOOP_TIMER_ID {
            let mut runnables = self.main_receiver.clone().try_iter();
            while let Some(Ok(runnable)) = runnables.next() {
                WindowsDispatcher::execute_runnable(runnable);
            }
            self.handle_paint_msg(handle)
        } else {
            None
        }
    }

    fn handle_paint_msg(&self, handle: HWND) -> Option<isize> {
        self.draw_window(handle, false)
    }

    fn handle_close_msg(&self) -> Option<isize> {
        let should_close = self.state.callbacks.should_close.invoke();
        if !should_close {
            return Some(0);
        }
        let shutdown = self.presentation_shutdown_ticket();
        if self.quiesce_presentation(&shutdown) == PlatformPresentationShutdownOutcome::Quiesced {
            None
        } else {
            Some(0)
        }
    }

    fn handle_destroy_msg(&self) -> Option<isize> {
        self.mark_native_window_destroying();
        let callback = { self.state.callbacks.close.take() };
        self.release_modal_parent();

        if let Some(callback) = callback {
            callback();
        }
        Some(0)
    }

    fn handle_mouse_move_msg(&self, handle: HWND, lparam: LPARAM, wparam: WPARAM) -> Option<isize> {
        let hover_started = self.start_tracking_mouse(handle, TME_LEAVE);
        self.restore_cursor_after_hide();

        let pressed_button = match MODIFIERKEYS_FLAGS(wparam.loword() as u32) {
            flags if flags.contains(MK_LBUTTON) => Some(MouseButton::Left),
            flags if flags.contains(MK_RBUTTON) => Some(MouseButton::Right),
            flags if flags.contains(MK_MBUTTON) => Some(MouseButton::Middle),
            flags if flags.contains(MK_XBUTTON1) => {
                Some(MouseButton::Navigate(NavigationDirection::Back))
            }
            flags if flags.contains(MK_XBUTTON2) => {
                Some(MouseButton::Navigate(NavigationDirection::Forward))
            }
            _ => None,
        };
        let client_position = point(
            DevicePixels(i32::from(lparam.signed_loword())),
            DevicePixels(i32::from(lparam.signed_hiword())),
        );
        let physical_frame = self.native_pointer_physical_frame_scope(Some(client_position), None);
        let scale_factor = physical_frame
            .frame()
            .map(|frame| frame.source_geometry().scale_factor())
            .unwrap_or_else(|| self.state.scale_factor.get());
        let position = logical_point(
            client_position.x.0 as f32,
            client_position.y.0 as f32,
            scale_factor,
        );
        self.state.last_client_pointer_position.set(Some(position));
        let input = PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button,
            modifiers: current_modifiers(),
        });
        let result = self.dispatch_input(input);
        drop(physical_frame);
        if hover_started {
            self.publish_hovered_status_change(true);
        }
        let handled = !result.propagate;

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_mouse_leave_msg(&self) -> Option<isize> {
        if !self.state.hovered.get() {
            return Some(1);
        }
        let Some(position) = self.state.last_client_pointer_position.get() else {
            log::error!("ignored native mouse-leave input without a preceding pointer position");
            self.state.hovered.set(false);
            self.state.cursor_visible.store(true, Ordering::Relaxed);
            self.publish_hovered_status_change(false);
            return Some(1);
        };
        // Win32 leave messages carry no pointer coordinates. Reuse only the last callback-owned
        // local position so hover can retire without fabricating a captured routing point.
        let input_result = self.dispatch_input(PlatformInput::MouseExited(MouseExitEvent {
            position,
            pressed_button: self.state.pointer_capture.get().pressed_button(),
            modifiers: current_modifiers(),
        }));

        self.state.hovered.set(false);
        // The next window's `WM_SETCURSOR` picks its own cursor, so we just clear
        // the flag for tight `is_cursor_visible()` semantics.
        self.state.cursor_visible.store(true, Ordering::Relaxed);
        let _ = with_windows_callback(&self.state.callbacks.hovered_status_change, |callback| {
            callback(false)
        });

        if input_result.propagate {
            Some(1)
        } else {
            Some(0)
        }
    }

    fn handle_syskeyup_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let input = handle_key_event(wparam, lparam, &self.state, |keystroke, _| {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        })?;
        self.dispatch_input(input);

        // Always return 0 to indicate that the message was handled, so we could properly handle `ModifiersChanged` event.
        Some(0)
    }

    // It's a known bug that you can't trigger `ctrl-shift-0`. See:
    // https://superuser.com/questions/1455762/ctrl-shift-number-key-combination-has-stopped-working-for-a-few-numbers
    fn handle_keydown_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let Some(input) = handle_key_event(
            wparam,
            lparam,
            &self.state,
            |keystroke, prefer_character_input| {
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke,
                    is_held: lparam.0 & (0x1 << 30) > 0,
                    prefer_character_input,
                })
            },
        ) else {
            return Some(1);
        };

        let result = self.dispatch_input(input);
        let handled = !result.propagate;

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_keyup_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let Some(input) = handle_key_event(wparam, lparam, &self.state, |keystroke, _| {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        }) else {
            return Some(1);
        };

        let result = self.dispatch_input(input);
        let handled = !result.propagate;

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_char_msg(&self, wparam: WPARAM) -> Option<isize> {
        let input = self.parse_char_message(wparam)?;
        self.with_input_handler(|input_handler| input_handler.replace_text_in_range(None, &input))?;

        Some(0)
    }

    fn handle_client_mouse_button_msg(
        &self,
        handle: HWND,
        message: WindowsClientMouseButtonMessage,
        lparam: LPARAM,
    ) -> Option<isize> {
        let client_position = point(
            DevicePixels(i32::from(lparam.signed_loword())),
            DevicePixels(i32::from(lparam.signed_hiword())),
        );
        let physical_frame = self.native_pointer_physical_frame_scope(Some(client_position), None);
        let button = message.button();
        let click_count = match message {
            WindowsClientMouseButtonMessage::Down(_) => {
                self.state.click_state.update(button, client_position)
            }
            WindowsClientMouseButtonMessage::Up(_) => self.state.click_state.current_count.get(),
        };
        let scale_factor = physical_frame
            .frame()
            .map(|frame| frame.source_geometry().scale_factor())
            .unwrap_or_else(|| self.state.scale_factor.get());
        let position = logical_point(
            client_position.x.0 as f32,
            client_position.y.0 as f32,
            scale_factor,
        );
        self.state.last_client_pointer_position.set(Some(position));
        let outcome = self.client_pointer_boundary().handle_button_message(
            message,
            position,
            current_modifiers(),
            click_count,
            |effect| self.apply_native_pointer_capture_effect(handle, effect),
        );
        if outcome.capture_acquisition_failed {
            self.settle_failed_native_pointer_capture_acquisition(handle);
        }
        outcome.dispatch_result
    }

    fn handle_pointer_capture_lost_msg(&self) -> Option<isize> {
        self.invalidate_native_pointer_physical_frame_scopes();
        self.non_client_pointer_boundary()
            .handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost)
            .canceled
            .then_some(0)
    }

    fn handle_cancel_mode_msg(&self, handle: HWND) -> Option<isize> {
        self.invalidate_native_pointer_physical_frame_scopes();
        let outcome = self
            .non_client_pointer_boundary()
            .handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        if outcome.release_native_capture && unsafe { GetCapture() } == handle {
            unsafe { ReleaseCapture().log_err() };
        }
        None
    }

    fn handle_mouse_wheel_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let modifiers = current_modifiers();

        let scale_factor = self.state.scale_factor.get();
        let wheel_scroll_amount = match modifiers.shift {
            true => self
                .system_settings()
                .mouse_wheel_settings
                .wheel_scroll_chars
                .get(),
            false => self
                .system_settings()
                .mouse_wheel_settings
                .wheel_scroll_lines
                .get(),
        };

        let wheel_distance =
            (wparam.signed_hiword() as f32 / WHEEL_DELTA as f32) * wheel_scroll_amount as f32;
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let position = logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor);
        self.state.last_client_pointer_position.set(Some(position));
        let input = PlatformInput::ScrollWheel(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Lines(match modifiers.shift {
                true => Point {
                    x: wheel_distance,
                    y: 0.0,
                },
                false => Point {
                    y: wheel_distance,
                    x: 0.0,
                },
            }),
            modifiers,
            touch_phase: TouchPhase::Moved,
        });
        let result = self.dispatch_input(input);
        let handled = !result.propagate;

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_mouse_horizontal_wheel_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let scale_factor = self.state.scale_factor.get();
        let wheel_scroll_chars = self
            .system_settings()
            .mouse_wheel_settings
            .wheel_scroll_chars
            .get();

        let wheel_distance =
            (-wparam.signed_hiword() as f32 / WHEEL_DELTA as f32) * wheel_scroll_chars as f32;
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let position = logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor);
        self.state.last_client_pointer_position.set(Some(position));
        let event = PlatformInput::ScrollWheel(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Lines(Point {
                x: wheel_distance,
                y: 0.0,
            }),
            modifiers: current_modifiers(),
            touch_phase: TouchPhase::Moved,
        });
        let result = self.dispatch_input(event);
        let handled = !result.propagate;

        if handled { Some(0) } else { Some(1) }
    }

    fn retrieve_caret_position(&self) -> Option<POINT> {
        self.with_input_handler_and_scale_factor(|input_handler, scale_factor| {
            let caret_range = input_handler.selected_text_range(false)?;
            let caret_position = input_handler.bounds_for_range(caret_range.range)?;
            Some(POINT {
                // logical to physical
                x: (caret_position.origin.x.as_f32() * scale_factor) as i32,
                y: (caret_position.origin.y.as_f32() * scale_factor) as i32
                    + ((caret_position.size.height.as_f32() * scale_factor) as i32 / 2),
            })
        })
    }

    fn handle_ime_position(&self, handle: HWND) -> Option<isize> {
        if let Some(caret_position) = self.retrieve_caret_position() {
            self.update_ime_position(handle, caret_position);
        }
        Some(0)
    }

    pub(crate) fn update_ime_position(&self, handle: HWND, caret_position: POINT) {
        let Some(ctx) = ImeContext::get(handle) else {
            return;
        };
        unsafe {
            ImmSetCompositionWindow(
                *ctx,
                &COMPOSITIONFORM {
                    dwStyle: CFS_POINT,
                    ptCurrentPos: caret_position,
                    ..Default::default()
                },
            )
            .ok()
            .log_err();

            ImmSetCandidateWindow(
                *ctx,
                &CANDIDATEFORM {
                    dwStyle: CFS_CANDIDATEPOS,
                    ptCurrentPos: caret_position,
                    ..Default::default()
                },
            )
            .ok()
            .log_err();
        }
    }

    fn update_ime_enabled(&self, handle: HWND) {
        let ime_enabled = self
            .with_input_handler(|input_handler| input_handler.query_accepts_text_input())
            .unwrap_or(false);
        if ime_enabled == self.state.ime_enabled.get() {
            return;
        }
        self.state.ime_enabled.set(ime_enabled);
        unsafe {
            if ime_enabled {
                ImmAssociateContextEx(handle, HIMC::default(), IACE_DEFAULT)
                    .ok()
                    .log_err();
            } else {
                if let Some(ctx) = ImeContext::get(handle) {
                    ImmNotifyIME(*ctx, NI_COMPOSITIONSTR, CPS_COMPLETE, 0)
                        .ok()
                        .log_err();
                }
                ImmAssociateContextEx(handle, HIMC::default(), 0)
                    .ok()
                    .log_err();
            }
        }
    }

    fn handle_ime_composition(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let ctx = ImeContext::get(handle)?;
        self.handle_ime_composition_inner(*ctx, lparam)
    }

    fn handle_ime_composition_inner(&self, ctx: HIMC, lparam: LPARAM) -> Option<isize> {
        let lparam = lparam.0 as u32;
        if lparam == 0 {
            // Japanese IME may send this message with lparam = 0, which indicates that
            // there is no composition string.
            self.with_input_handler(|input_handler| input_handler.replace_text_in_range(None, ""))?;
            Some(0)
        } else {
            if lparam & GCS_RESULTSTR.0 > 0 {
                let comp_result = parse_ime_composition_string(ctx, GCS_RESULTSTR)?;
                self.with_input_handler(|input_handler| {
                    input_handler
                        .replace_text_in_range(None, &String::from_utf16_lossy(&comp_result))
                })?;
            }
            if lparam & GCS_COMPSTR.0 > 0 {
                let comp_string = parse_ime_composition_string(ctx, GCS_COMPSTR)?;
                let caret_pos =
                    (!comp_string.is_empty() && lparam & GCS_CURSORPOS.0 > 0).then(|| {
                        let cursor_pos = retrieve_composition_cursor_position(ctx);
                        let pos = if should_use_ime_cursor_position(ctx, cursor_pos) {
                            cursor_pos
                        } else {
                            comp_string.len()
                        };
                        pos..pos
                    });
                self.with_input_handler(|input_handler| {
                    input_handler.replace_and_mark_text_in_range(
                        None,
                        &String::from_utf16_lossy(&comp_string),
                        caret_pos,
                    )
                })?;
            }
            if lparam & (GCS_RESULTSTR.0 | GCS_COMPSTR.0) > 0 {
                return Some(0);
            }

            // currently, we don't care other stuff
            None
        }
    }

    fn handle_calc_client_size(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        if !self.hide_title_bar || self.state.is_fullscreen() || wparam.0 == 0 {
            return None;
        }

        unsafe {
            let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
            let saved_top = (*params).rgrc[0].top;
            let result = DefWindowProcW(handle, WM_NCCALCSIZE, wparam, lparam);
            (*params).rgrc[0].top = saved_top;
            if self.state.is_maximized() {
                let dpi = GetDpiForWindow(handle);
                (*params).rgrc[0].top += get_frame_thicknessx(dpi);
            }
            Some(result.0 as isize)
        }
    }

    fn handle_activate_msg(self: &Rc<Self>, handle: HWND, wparam: WPARAM) -> Option<isize> {
        let activated = wparam.loword() > 0;

        if activated && !self.provisional_accepts_interaction() {
            return Some(0);
        }

        if !activated {
            self.invalidate_native_pointer_physical_frame_scopes();
            self.state.cursor_visible.store(true, Ordering::Relaxed);
            // ActiveChanged remains the cross-platform pointer-cancellation authority. Retire
            // Windows bookkeeping and OS capture before any fallible notification so a panic
            // cannot strand capture on the deactivated HWND.
            let outcome = self
                .non_client_pointer_boundary()
                .clear_pointer_capture(PointerCancelReason::WindowDeactivated);
            if outcome.release_native_capture && unsafe { GetCapture() } == handle {
                unsafe { ReleaseCapture().log_err() };
            }
        }

        let events = self
            .state
            .a11y
            .try_borrow_mut()
            .ok()
            .and_then(|mut a11y| a11y.as_mut()?.adapter.update_window_focus_state(activated));
        if let Some(events) = events {
            events.raise();
        }

        let _ = with_windows_callback(&self.state.callbacks.active_status_change, |callback| {
            callback(activated)
        });

        // When the window is activated (gains focus), reset the modifier tracking state.
        // This fixes the issue where Alt-Tab away and back leaves stale modifier state
        // (especially the Alt key) because Windows doesn't always send key-up events to
        // windows that have lost focus.
        if activated {
            self.state.last_reported_modifiers.set(None);
            self.state.last_reported_capslock.set(None);

            let event = ModifiersChangedEvent {
                modifiers: current_modifiers(),
                capslock: current_capslock(),
            };
            let _ = with_windows_callback(&self.state.callbacks.modifiers_changed, |callback| {
                callback(event)
            });
        }

        None
    }

    fn handle_wm_getobject(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let result = {
            let mut a11y = self.state.a11y.borrow_mut();
            let a11y = a11y.as_mut()?;
            a11y.adapter.handle_wm_getobject(
                accesskit_windows::WPARAM(wparam.0),
                accesskit_windows::LPARAM(lparam.0),
                &mut a11y.activation_handler,
            )?
        };
        // The borrow above must be dropped before calling `.into()`, because
        // it calls `UiaReturnRawElementProvider` which may send a nested
        // `WM_GETOBJECT` back into this window procedure.
        let lresult: accesskit_windows::LRESULT = result.into();
        Some(lresult.0)
    }

    fn handle_create_msg(&self, handle: HWND) -> Option<isize> {
        if self.hide_title_bar {
            notify_frame_changed(handle);
            Some(0)
        } else {
            None
        }
    }

    fn handle_dpi_changed_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let new_dpi = wparam.loword() as f32;

        let is_maximized = self.state.is_maximized();
        let new_scale_factor = new_dpi / USER_DEFAULT_SCREEN_DPI as f32;
        self.state.scale_factor.set(new_scale_factor);
        self.state.border_offset.update(handle).log_err();

        self.state
            .direct_manipulation
            .set_scale_factor(new_scale_factor);

        if is_maximized {
            // Get the monitor and its work area at the new DPI
            let monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST) };
            let mut monitor_info: MONITORINFO = unsafe { std::mem::zeroed() };
            monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool() {
                let work_area = monitor_info.rcWork;
                let width = work_area.right - work_area.left;
                let height = work_area.bottom - work_area.top;

                // Update the window size to match the new monitor work area
                // This will trigger WM_SIZE which will handle the size change
                unsafe {
                    SetWindowPos(
                        handle,
                        None,
                        work_area.left,
                        work_area.top,
                        width,
                        height,
                        SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    )
                    .context("unable to set maximized window position after dpi has changed")
                    .log_err();
                }

                // SetWindowPos may not send WM_SIZE for maximized windows in some cases,
                // so we manually update the size to ensure proper rendering
                let device_size = size(DevicePixels(width), DevicePixels(height));
                self.handle_size_change(device_size, new_scale_factor, true);
            }
        } else {
            // For non-maximized windows, use the suggested RECT from the system
            let rect = unsafe { &*(lparam.0 as *const RECT) };
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            // this will emit `WM_SIZE` and `WM_MOVE` right here
            // even before this function returns
            // the new size is handled in `WM_SIZE`
            unsafe {
                SetWindowPos(
                    handle,
                    None,
                    rect.left,
                    rect.top,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
                )
                .context("unable to set window position after dpi has changed")
                .log_err();
            }
        }

        Some(0)
    }

    fn handle_display_change_msg(&self, handle: HWND) -> Option<isize> {
        let new_monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONULL) };
        if new_monitor.is_invalid() {
            log::error!("No monitor detected!");
            return None;
        }
        let new_display = WindowsDisplay::new(WindowsDisplay::display_id_for_monitor(new_monitor))?;
        self.state.display.set(new_display);
        Some(0)
    }

    fn handle_hit_test_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        if self.provisional_requires_hit_transparency() || !self.state.accepts_pointer_input() {
            return Some(HTTRANSPARENT as _);
        }

        if self.state.is_fullscreen() {
            return None;
        }

        let drag_area =
            with_windows_callback(&self.state.callbacks.hit_test_window_control, |callback| {
                callback()
            })
            .flatten()
            .and_then(|area| hit_test_window_control_area(area, self.is_movable));

        if !self.hide_title_bar {
            // If the OS draws the title bar, we don't need to handle hit test messages.
            return drag_area;
        }

        let dpi = unsafe { GetDpiForWindow(handle) };
        // We do not use the OS title bar, so the default `DefWindowProcW` will only register a 1px edge for resizes
        // We need to calculate the frame thickness ourselves and do the hit test manually.
        let frame_y = get_frame_thicknessx(dpi);
        let frame_x = get_frame_thicknessy(dpi);
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };

        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        if !self.state.is_maximized() && 0 <= cursor_point.y && cursor_point.y <= frame_y {
            // x-axis actually goes from -frame_x to 0
            return Some(if cursor_point.x <= 0 {
                HTTOPLEFT
            } else {
                let mut rect = Default::default();
                unsafe { GetWindowRect(handle, &mut rect) }.log_err();
                // right and bottom bounds of RECT are exclusive, thus `-1`
                let right = rect.right - rect.left - 1;
                // the bounds include the padding frames, so accomodate for both of them
                if right - 2 * frame_x <= cursor_point.x {
                    HTTOPRIGHT
                } else {
                    HTTOP
                }
            } as _);
        }

        drag_area
    }

    fn handle_nc_mouse_move_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let hover_started = self.start_tracking_mouse(handle, TME_LEAVE | TME_NONCLIENT);
        self.restore_cursor_after_hide();

        let scale_factor = self.state.scale_factor.get();

        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let position = logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor);
        self.state.last_client_pointer_position.set(Some(position));
        let input = PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: current_modifiers(),
        });
        let handled = !self.dispatch_input(input).propagate;
        if hover_started {
            self.publish_hovered_status_change(true);
        }

        if handled { Some(0) } else { None }
    }

    fn handle_nc_mouse_down_msg(
        &self,
        handle: HWND,
        button: MouseButton,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let scale_factor = self.state.scale_factor.get();
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let physical_point = point(DevicePixels(cursor_point.x), DevicePixels(cursor_point.y));
        let click_count = self.state.click_state.update(button, physical_point);

        let position = logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor);
        self.state.last_client_pointer_position.set(Some(position));
        let outcome = self.non_client_pointer_boundary().handle_button_message(
            WindowsClientMouseButtonMessage::Down(button),
            wparam.0 as u32,
            position,
            current_modifiers(),
            click_count,
            |effect| self.apply_native_pointer_capture_effect(handle, effect),
        );
        if outcome.capture_acquisition_failed {
            self.settle_failed_native_pointer_capture_acquisition(handle);
        }
        outcome.consumed.then_some(0)
    }

    fn handle_nc_mouse_up_msg(
        &self,
        handle: HWND,
        button: MouseButton,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let scale_factor = self.state.scale_factor.get();

        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let position = logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor);
        self.state.last_client_pointer_position.set(Some(position));
        let outcome = self.non_client_pointer_boundary().handle_button_message(
            WindowsClientMouseButtonMessage::Up(button),
            wparam.0 as u32,
            position,
            current_modifiers(),
            1,
            |effect| self.apply_native_pointer_capture_effect(handle, effect),
        );
        if outcome.capture_acquisition_failed {
            self.settle_failed_native_pointer_capture_acquisition(handle);
        }
        if let Some(caption_action) = outcome.caption_action {
            match caption_action {
                WindowsCaptionButtonAction::Minimize => {
                    unsafe { ShowWindowAsync(handle, SW_MINIMIZE).ok().log_err() };
                }
                WindowsCaptionButtonAction::ToggleMaximize => {
                    if self.state.is_maximized() {
                        unsafe { ShowWindowAsync(handle, SW_NORMAL).ok().log_err() };
                    } else {
                        unsafe { ShowWindowAsync(handle, SW_MAXIMIZE).ok().log_err() };
                    }
                }
                WindowsCaptionButtonAction::Close => {
                    unsafe {
                        PostMessageW(Some(handle), WM_CLOSE, WPARAM::default(), LPARAM::default())
                            .log_err()
                    };
                }
            }
        }
        outcome.consumed.then_some(0)
    }

    fn handle_set_cursor(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        if unsafe { !IsWindowEnabled(handle).as_bool() }
            || matches!(
                lparam.loword() as u32,
                HTLEFT
                    | HTRIGHT
                    | HTTOP
                    | HTTOPLEFT
                    | HTTOPRIGHT
                    | HTBOTTOM
                    | HTBOTTOMLEFT
                    | HTBOTTOMRIGHT
            )
        {
            return None;
        }
        let cursor = if self.state.cursor_visible.load(Ordering::Relaxed) {
            self.state.current_cursor.get()
        } else {
            None
        };
        unsafe {
            SetCursor(cursor);
        };
        Some(0)
    }

    fn handle_system_settings_changed(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        if wparam.0 != 0 {
            self.state.click_state.system_update(wparam.0);
            self.state.border_offset.update(handle).log_err();
            // system settings may emit a window message which wants to take the refcell self.state, so drop it

            self.system_settings().update(wparam.0);
        } else {
            self.handle_system_theme_changed(handle, lparam)?;
        };

        Some(0)
    }

    fn handle_system_theme_changed(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        // lParam is a pointer to a string that indicates the area containing the system parameter
        // that was changed.
        let parameter = PCWSTR::from_raw(lparam.0 as _);
        if unsafe { !parameter.is_null() && !parameter.is_empty() }
            && let Some(parameter_string) = unsafe { parameter.to_string() }.log_err()
        {
            log::info!("System settings changed: {}", parameter_string);
            if parameter_string.as_str() == "ImmersiveColorSet" {
                let new_appearance = system_appearance()
                    .context("unable to get system appearance when handling ImmersiveColorSet")
                    .log_err()?;

                if new_appearance != self.state.appearance.get() {
                    self.state.appearance.set(new_appearance);
                    with_windows_callback(&self.state.callbacks.appearance_changed, |callback| {
                        callback()
                    })?;
                    configure_dwm_dark_mode(handle, new_appearance);
                }
            }
        }
        Some(0)
    }

    fn handle_input_language_changed(&self) -> Option<isize> {
        unsafe {
            PostMessageW(
                Some(self.platform_window_handle),
                WM_GPUI_KEYBOARD_LAYOUT_CHANGED,
                WPARAM(self.validation_number),
                LPARAM(0),
            )
            .log_err();
        }
        Some(0)
    }

    fn handle_window_visibility_changed(&self, handle: HWND, wparam: WPARAM) -> Option<isize> {
        if wparam.0 == 1 {
            self.draw_window(handle, false);
        }
        None
    }

    fn handle_device_lost(&self) -> Option<isize> {
        if self.presentation_shutdown_claimed() {
            return Some(0);
        }
        let devices = self.recovered_directx_devices.read().clone()?;
        if let Err(err) = self
            .state
            .renderer
            .borrow_mut()
            .handle_device_lost(&devices)
        {
            log::error!("Failed to refresh window renderer after device lost: {err:?}");
            self.state.invalidate_devices.store(true, Ordering::Release);
            return Some(0);
        }
        Some(0)
    }

    fn handle_dm_pointer_hit_test(&self, wparam: WPARAM) -> Option<isize> {
        self.state.direct_manipulation.on_pointer_hit_test(wparam);
        None
    }

    #[inline]
    fn draw_window(&self, handle: HWND, force_render: bool) -> Option<isize> {
        with_windows_callback(&self.state.callbacks.request_frame, |request_frame| {
            self.state.direct_manipulation.update();

            let events = self.state.direct_manipulation.drain_events();
            for event in events {
                let _ = self.dispatch_input(event);
            }

            request_frame(RequestFrameOptions {
                require_presentation: false,
                force_render,
            });

            self.update_ime_enabled(handle);
            unsafe { ValidateRect(Some(handle), None).ok().log_err() };
        })?;

        Some(0)
    }

    #[inline]
    fn parse_char_message(&self, wparam: WPARAM) -> Option<String> {
        let code_point = wparam.loword();

        // https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G2630
        match code_point {
            0xD800..=0xDBFF => {
                // High surrogate, wait for low surrogate
                self.state.pending_surrogate.set(Some(code_point));
                None
            }
            0xDC00..=0xDFFF => {
                if let Some(high_surrogate) = self.state.pending_surrogate.take() {
                    // Low surrogate, combine with pending high surrogate
                    String::from_utf16(&[high_surrogate, code_point]).ok()
                } else {
                    // Invalid low surrogate without a preceding high surrogate
                    log::warn!(
                        "Received low surrogate without a preceding high surrogate: {code_point:x}"
                    );
                    None
                }
            }
            _ => {
                self.state.pending_surrogate.set(None);
                char::from_u32(code_point as u32)
                    .filter(|c| !c.is_control())
                    .map(|c| c.to_string())
            }
        }
    }

    /// Clear the hidden flag and restore the cursor immediately
    fn restore_cursor_after_hide(&self) {
        if !self.state.cursor_visible.swap(true, Ordering::Relaxed) {
            unsafe {
                SetCursor(self.state.current_cursor.get());
            }
        }
    }

    fn start_tracking_mouse(&self, handle: HWND, flags: TRACKMOUSEEVENT_FLAGS) -> bool {
        if !self.state.hovered.get() {
            self.state.hovered.set(true);
            unsafe {
                TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: flags,
                    hwndTrack: handle,
                    dwHoverTime: HOVER_DEFAULT,
                })
                .log_err()
            };
            true
        } else {
            false
        }
    }

    fn publish_hovered_status_change(&self, hovered: bool) {
        let _ = with_windows_callback(&self.state.callbacks.hovered_status_change, |callback| {
            callback(hovered)
        });
    }

    fn with_input_handler<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PlatformInputHandler) -> R,
    {
        if !self.provisional_accepts_interaction() {
            return None;
        }
        self.state.input_handler.with_handler(f)
    }

    fn with_input_handler_and_scale_factor<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PlatformInputHandler, f32) -> Option<R>,
    {
        if !self.provisional_accepts_interaction() {
            return None;
        }
        let scale_factor = self.state.scale_factor.get();
        self.state
            .input_handler
            .with_handler(|input_handler| f(input_handler, scale_factor))
            .flatten()
    }
}

struct ImeContext {
    hwnd: HWND,
    himc: HIMC,
}

impl ImeContext {
    fn get(hwnd: HWND) -> Option<Self> {
        let himc = unsafe { ImmGetContext(hwnd) };
        if himc.is_invalid() {
            return None;
        }
        Some(Self { hwnd, himc })
    }
}

impl std::ops::Deref for ImeContext {
    type Target = HIMC;
    fn deref(&self) -> &HIMC {
        &self.himc
    }
}

impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe {
            ImmReleaseContext(self.hwnd, self.himc).ok().log_err();
        }
    }
}

fn handle_key_event<F>(
    wparam: WPARAM,
    lparam: LPARAM,
    state: &WindowsWindowState,
    f: F,
) -> Option<PlatformInput>
where
    F: FnOnce(Keystroke, bool) -> PlatformInput,
{
    let virtual_key = VIRTUAL_KEY(wparam.loword());
    let modifiers = current_modifiers();

    match virtual_key {
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN => {
            if state
                .last_reported_modifiers
                .get()
                .is_some_and(|prev_modifiers| prev_modifiers == modifiers)
            {
                return None;
            }
            state.last_reported_modifiers.set(Some(modifiers));
            Some(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock: current_capslock(),
            }))
        }
        VK_PACKET => None,
        VK_CAPITAL => {
            let capslock = current_capslock();
            if state
                .last_reported_capslock
                .get()
                .is_some_and(|prev_capslock| prev_capslock == capslock)
            {
                return None;
            }
            state.last_reported_capslock.set(Some(capslock));
            Some(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }))
        }
        vkey => {
            let keystroke = parse_normal_key(vkey, lparam, modifiers)?;
            Some(f(keystroke.0, keystroke.1))
        }
    }
}

fn parse_immutable(vkey: VIRTUAL_KEY) -> Option<String> {
    Some(
        match vkey {
            VK_SPACE => "space",
            VK_BACK => "backspace",
            VK_RETURN => "enter",
            VK_TAB => "tab",
            VK_UP => "up",
            VK_DOWN => "down",
            VK_RIGHT => "right",
            VK_LEFT => "left",
            VK_HOME => "home",
            VK_END => "end",
            VK_PRIOR => "pageup",
            VK_NEXT => "pagedown",
            VK_BROWSER_BACK => "back",
            VK_BROWSER_FORWARD => "forward",
            VK_ESCAPE => "escape",
            VK_INSERT => "insert",
            VK_DELETE => "delete",
            VK_APPS => "menu",
            VK_F1 => "f1",
            VK_F2 => "f2",
            VK_F3 => "f3",
            VK_F4 => "f4",
            VK_F5 => "f5",
            VK_F6 => "f6",
            VK_F7 => "f7",
            VK_F8 => "f8",
            VK_F9 => "f9",
            VK_F10 => "f10",
            VK_F11 => "f11",
            VK_F12 => "f12",
            VK_F13 => "f13",
            VK_F14 => "f14",
            VK_F15 => "f15",
            VK_F16 => "f16",
            VK_F17 => "f17",
            VK_F18 => "f18",
            VK_F19 => "f19",
            VK_F20 => "f20",
            VK_F21 => "f21",
            VK_F22 => "f22",
            VK_F23 => "f23",
            VK_F24 => "f24",
            _ => return None,
        }
        .to_string(),
    )
}

fn parse_normal_key(
    vkey: VIRTUAL_KEY,
    lparam: LPARAM,
    mut modifiers: Modifiers,
) -> Option<(Keystroke, bool)> {
    let (key_char, prefer_character_input) = process_key(vkey, lparam.hiword());

    let key = parse_immutable(vkey).or_else(|| {
        let scan_code = lparam.hiword() & 0xFF;
        get_keystroke_key(vkey, scan_code as u32, &mut modifiers)
    })?;

    Some((
        Keystroke {
            modifiers,
            key,
            key_char,
        },
        prefer_character_input,
    ))
}

fn process_key(vkey: VIRTUAL_KEY, scan_code: u16) -> (Option<String>, bool) {
    let mut keyboard_state = [0u8; 256];
    unsafe {
        if GetKeyboardState(&mut keyboard_state).is_err() {
            return (None, false);
        }
    }

    let mut buffer_c = [0u16; 8];
    let result_c = unsafe {
        ToUnicode(
            vkey.0 as u32,
            scan_code as u32,
            Some(&keyboard_state),
            &mut buffer_c,
            0x4,
        )
    };

    if result_c == 0 {
        return (None, false);
    }

    let c = &buffer_c[..result_c.unsigned_abs() as usize];
    let key_char = String::from_utf16(c)
        .ok()
        .filter(|s| !s.is_empty() && !s.chars().next().unwrap().is_control());

    if result_c < 0 {
        return (key_char, true);
    }

    if key_char.is_none() {
        return (None, false);
    }

    // Workaround for some bug that makes the compiler think keyboard_state is still zeroed out
    let keyboard_state = std::hint::black_box(keyboard_state);
    let ctrl_down = (keyboard_state[VK_CONTROL.0 as usize] & 0x80) != 0;
    let alt_down = (keyboard_state[VK_MENU.0 as usize] & 0x80) != 0;
    let win_down = (keyboard_state[VK_LWIN.0 as usize] & 0x80) != 0
        || (keyboard_state[VK_RWIN.0 as usize] & 0x80) != 0;

    let has_modifiers = ctrl_down || alt_down || win_down;
    if !has_modifiers {
        return (key_char, false);
    }

    let mut state_no_modifiers = keyboard_state;
    state_no_modifiers[VK_CONTROL.0 as usize] = 0;
    state_no_modifiers[VK_LCONTROL.0 as usize] = 0;
    state_no_modifiers[VK_RCONTROL.0 as usize] = 0;
    state_no_modifiers[VK_MENU.0 as usize] = 0;
    state_no_modifiers[VK_LMENU.0 as usize] = 0;
    state_no_modifiers[VK_RMENU.0 as usize] = 0;
    state_no_modifiers[VK_LWIN.0 as usize] = 0;
    state_no_modifiers[VK_RWIN.0 as usize] = 0;

    let mut buffer_c_no_modifiers = [0u16; 8];
    let result_c_no_modifiers = unsafe {
        ToUnicode(
            vkey.0 as u32,
            scan_code as u32,
            Some(&state_no_modifiers),
            &mut buffer_c_no_modifiers,
            0x4,
        )
    };

    let c_no_modifiers = &buffer_c_no_modifiers[..result_c_no_modifiers.unsigned_abs() as usize];
    (
        key_char,
        result_c != result_c_no_modifiers || c != c_no_modifiers,
    )
}

fn parse_ime_composition_string(ctx: HIMC, comp_type: IME_COMPOSITION_STRING) -> Option<Vec<u16>> {
    unsafe {
        let string_len = ImmGetCompositionStringW(ctx, comp_type, None, 0);
        if string_len >= 0 {
            let mut buffer = vec![0u8; string_len as usize + 2];
            ImmGetCompositionStringW(
                ctx,
                comp_type,
                Some(buffer.as_mut_ptr() as _),
                string_len as _,
            );
            let wstring = std::slice::from_raw_parts::<u16>(
                buffer.as_mut_ptr().cast::<u16>(),
                string_len as usize / 2,
            );
            Some(wstring.to_vec())
        } else {
            None
        }
    }
}

#[inline]
fn retrieve_composition_cursor_position(ctx: HIMC) -> usize {
    unsafe { ImmGetCompositionStringW(ctx, GCS_CURSORPOS, None, 0) as usize }
}

fn should_use_ime_cursor_position(ctx: HIMC, cursor_pos: usize) -> bool {
    let attrs_size = unsafe { ImmGetCompositionStringW(ctx, GCS_COMPATTR, None, 0) } as usize;
    if attrs_size == 0 {
        return false;
    }

    let mut attrs = vec![0u8; attrs_size];
    let result = unsafe {
        ImmGetCompositionStringW(
            ctx,
            GCS_COMPATTR,
            Some(attrs.as_mut_ptr() as *mut _),
            attrs_size as u32,
        )
    };
    if result <= 0 {
        return false;
    }

    // Keep the cursor adjacent to the inserted text by only using the suggested position
    // if it's adjacent to unconverted text.
    let at_cursor_is_input = cursor_pos < attrs.len() && attrs[cursor_pos] == (ATTR_INPUT as u8);
    let before_cursor_is_input = cursor_pos > 0
        && (cursor_pos - 1) < attrs.len()
        && attrs[cursor_pos - 1] == (ATTR_INPUT as u8);

    at_cursor_is_input || before_cursor_is_input
}

#[inline]
fn is_virtual_key_pressed(vkey: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(vkey.0 as i32) < 0 }
}

#[inline]
pub(crate) fn current_modifiers() -> Modifiers {
    Modifiers {
        control: is_virtual_key_pressed(VK_CONTROL),
        alt: is_virtual_key_pressed(VK_MENU),
        shift: is_virtual_key_pressed(VK_SHIFT),
        platform: is_virtual_key_pressed(VK_LWIN) || is_virtual_key_pressed(VK_RWIN),
        function: false,
    }
}

#[inline]
pub(crate) fn current_capslock() -> Capslock {
    let on = unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 } > 0;
    Capslock { on }
}

// there is some additional non-visible space when talking about window
// borders on Windows:
// - SM_CXSIZEFRAME: The resize handle.
// - SM_CXPADDEDBORDER: Additional border space that isn't part of the resize handle.
fn get_frame_thicknessx(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn get_frame_thicknessy(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn hit_test_window_control_area(area: WindowControlArea, is_movable: bool) -> Option<isize> {
    match area {
        WindowControlArea::Drag if is_movable => Some(HTCAPTION as _),
        WindowControlArea::Drag => None,
        WindowControlArea::Close => Some(HTCLOSE as _),
        WindowControlArea::Max => Some(HTMAXBUTTON as _),
        WindowControlArea::Min => Some(HTMINBUTTON as _),
    }
}

fn notify_frame_changed(handle: HWND) {
    unsafe {
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOACTIVATE
                | SWP_NOCOPYBITS
                | SWP_NOMOVE
                | SWP_NOOWNERZORDER
                | SWP_NOREPOSITION
                | SWP_NOSENDCHANGING
                | SWP_NOSIZE
                | SWP_NOZORDER,
        )
        .log_err();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use open_gpui::{
        DispatchEventResult, Modifiers, MouseButton, PlatformInput, Point, PointerCancelReason,
        WindowControlArea,
    };
    use windows::Win32::{
        Foundation::WPARAM,
        UI::Controls::WM_MOUSELEAVE,
        UI::WindowsAndMessaging::{
            HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_NCMOUSELEAVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
        },
    };

    use super::{
        WindowsCaptionButtonAction, WindowsClientMouseButtonMessage, WindowsClientPointerBoundary,
        WindowsInputDispatchGuard, WindowsInputDispatchState,
        WindowsNativePointerCaptureReleaseState, WindowsNonClientPointerBoundary,
        WindowsPointerCaptureEffect, WindowsPointerCaptureInput, WindowsPointerCaptureState,
        decode_client_mouse_button_message, hit_test_window_control_area, may_own_pointer_session,
        should_reserve_pointer_cancel_after_callback_panic,
    };
    use crate::Callbacks;

    #[test]
    fn pointer_capture_tracks_first_companion_and_final_buttons() {
        let (state, effect) = WindowsPointerCaptureState::default().transition(
            WindowsPointerCaptureInput::ButtonDown {
                button: MouseButton::Left,
                acquire_native_capture: true,
            },
        );
        assert_eq!(effect, WindowsPointerCaptureEffect::Acquire);

        let (state, effect) = state.transition(WindowsPointerCaptureInput::ButtonDown {
            button: MouseButton::Right,
            acquire_native_capture: true,
        });
        assert_eq!(effect, WindowsPointerCaptureEffect::None);

        let (state, effect) =
            state.transition(WindowsPointerCaptureInput::ButtonUp(MouseButton::Left));
        assert_eq!(effect, WindowsPointerCaptureEffect::None);

        let (state, effect) =
            state.transition(WindowsPointerCaptureInput::ButtonUp(MouseButton::Right));
        assert_eq!(state, WindowsPointerCaptureState::default());
        assert_eq!(effect, WindowsPointerCaptureEffect::Release);
    }

    #[test]
    fn native_button_message_boundary_dispatches_companion_buttons_through_registered_callback() {
        #[derive(Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down(MouseButton),
            Up(MouseButton),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::MouseDown(event) => {
                        observed
                            .borrow_mut()
                            .push(ObservedInput::Down(event.button));
                    }
                    PlatformInput::MouseUp(event) => {
                        observed.borrow_mut().push(ObservedInput::Up(event.button));
                    }
                    _ => {}
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };
        let mut capture_effects = Vec::new();

        for message in [WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_RBUTTONUP] {
            boundary.handle_button_message(
                decode_client_mouse_button_message(message, WPARAM::default()).unwrap(),
                Point::default(),
                Modifiers::default(),
                1,
                |effect| {
                    capture_effects.push(effect);
                    true
                },
            );
        }
        assert_ne!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert_eq!(capture_effects, vec![WindowsPointerCaptureEffect::Acquire]);

        boundary.handle_button_message(
            decode_client_mouse_button_message(WM_LBUTTONUP, WPARAM::default()).unwrap(),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                capture_effects.push(effect);
                true
            },
        );

        assert_eq!(
            *observed.borrow(),
            vec![
                ObservedInput::Down(MouseButton::Left),
                ObservedInput::Down(MouseButton::Right),
                ObservedInput::Up(MouseButton::Right),
                ObservedInput::Up(MouseButton::Left),
            ]
        );
        assert_eq!(
            capture_effects,
            vec![
                WindowsPointerCaptureEffect::Acquire,
                WindowsPointerCaptureEffect::Release,
            ]
        );
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn stale_framework_release_cannot_target_a_newer_native_capture() {
        let releases = WindowsNativePointerCaptureReleaseState::default();

        releases.record_pointer_session_start();
        let first_session = releases.current_pointer_session_epoch();
        assert!(releases.matches_pointer_session(first_session));

        releases.record_pointer_session_start();
        assert!(
            !releases.matches_pointer_session(first_session),
            "a delayed first attempt or retry for the first capture must not target the replacement capture"
        );
        let second_session = releases.current_pointer_session_epoch();
        assert!(releases.matches_pointer_session(second_session));
    }

    #[test]
    fn stale_framework_release_cannot_target_a_newer_non_client_pointer_session() {
        let releases = WindowsNativePointerCaptureReleaseState::default();
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &releases,
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        boundary.pointer.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        let first_session = releases.current_pointer_session_epoch();
        assert!(releases.matches_pointer_session(first_session));

        let retired = boundary
            .pointer
            .clear_pointer_capture(PointerCancelReason::CaptureRevoked);
        assert!(retired.canceled);
        assert!(retired.release_native_capture);
        let capture_lost = boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        assert!(!capture_lost.canceled);

        let newer_down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        assert!(newer_down.consumed);
        assert_eq!(
            pressed_caption_button.get(),
            Some(WindowsCaptionButtonAction::Close)
        );

        assert!(
            !releases.matches_pointer_session(first_session),
            "a delayed first attempt or retry for the first session must not target the replacement non-client session"
        );
        assert!(pointer_capture.get().is_button_pressed(MouseButton::Left));
        assert_eq!(
            pressed_caption_button.get(),
            Some(WindowsCaptionButtonAction::Close)
        );
        let second_session = releases.current_pointer_session_epoch();
        assert!(releases.matches_pointer_session(second_session));
    }

    #[test]
    fn failed_native_capture_acquisition_orders_mouse_down_before_terminal_cancel() {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down,
            Cancel(PointerCancelReason),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::MouseDown(_) => observed.borrow_mut().push(ObservedInput::Down),
                    PlatformInput::PointerCanceled(event) => observed
                        .borrow_mut()
                        .push(ObservedInput::Cancel(event.reason)),
                    _ => {}
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };

        let down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                assert_eq!(effect, WindowsPointerCaptureEffect::Acquire);
                false
            },
        );
        assert!(down.capture_acquisition_failed);
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ObservedInput::Down,
                ObservedInput::Cancel(PointerCancelReason::CaptureRevoked),
            ]
        );
        let cleanup = boundary.clear_pointer_capture(PointerCancelReason::CaptureRevoked);
        assert!(cleanup.canceled);
        assert_eq!(observed.borrow().len(), 2);
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn capture_loss_reentrant_to_acquisition_is_deferred_until_after_mouse_down() {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down,
            Cancel(PointerCancelReason),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::MouseDown(_) => observed.borrow_mut().push(ObservedInput::Down),
                    PlatformInput::PointerCanceled(event) => observed
                        .borrow_mut()
                        .push(ObservedInput::Cancel(event.reason)),
                    _ => {}
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };

        let down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                assert_eq!(effect, WindowsPointerCaptureEffect::Acquire);
                let cancel =
                    boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
                assert!(cancel.canceled);
                assert!(cancel.release_native_capture);
                assert!(observed.borrow().is_empty());
                false
            },
        );

        assert!(down.capture_acquisition_failed);
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ObservedInput::Down,
                ObservedInput::Cancel(PointerCancelReason::PlatformCaptureLost),
            ]
        );
        let cleanup = boundary.clear_pointer_capture(PointerCancelReason::CaptureRevoked);
        assert!(cleanup.canceled);
        assert_eq!(observed.borrow().len(), 2);
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn native_capture_loss_boundary_dispatches_one_terminal_cancel() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                if let PlatformInput::PointerCanceled(event) = input {
                    observed.borrow_mut().push(event.reason);
                }
                DispatchEventResult::default()
            }
        }));
        let (active_capture, _) = WindowsPointerCaptureState::default().transition(
            WindowsPointerCaptureInput::ButtonDown {
                button: MouseButton::Left,
                acquire_native_capture: true,
            },
        );
        let pointer_capture = Cell::new(active_capture);
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };

        let cancel = boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        assert!(cancel.canceled);
        assert!(cancel.release_native_capture);
        let duplicate = boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        assert!(!duplicate.canceled);
        assert!(!duplicate.release_native_capture);

        assert_eq!(
            observed.borrow().as_slice(),
            &[PointerCancelReason::PlatformCaptureLost]
        );
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn non_client_button_session_cancels_once_without_acquiring_native_capture() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::MouseDown(event) => {
                        observed.borrow_mut().push(Ok(event.button));
                    }
                    PlatformInput::PointerCanceled(event) => {
                        observed.borrow_mut().push(Err(event.reason));
                    }
                    _ => {}
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };
        let mut native_effects = Vec::new();

        boundary.handle_non_client_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );

        assert_ne!(pointer_capture.get(), WindowsPointerCaptureState::default());
        assert!(native_effects.is_empty());
        let cancel = boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        assert!(cancel.canceled);
        assert!(!cancel.release_native_capture);
        let duplicate = boundary.handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost);
        assert!(!duplicate.canceled);
        assert!(!duplicate.release_native_capture);
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                Ok(MouseButton::Left),
                Err(PointerCancelReason::PlatformCaptureLost),
            ]
        );
    }

    #[test]
    fn non_client_button_session_never_changes_native_capture() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };
        let mut native_effects = Vec::new();

        for message in [
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
        ] {
            boundary.handle_non_client_button_message(
                message,
                Point::default(),
                Modifiers::default(),
                1,
                |effect| {
                    native_effects.push(effect);
                    true
                },
            );
        }

        assert!(native_effects.is_empty());
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
    }

    #[test]
    fn client_owned_capture_releases_when_final_up_arrives_non_client() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };
        let mut native_effects = Vec::new();

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );
        boundary.handle_non_client_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );

        assert_eq!(
            native_effects,
            [
                WindowsPointerCaptureEffect::Acquire,
                WindowsPointerCaptureEffect::Release,
            ]
        );
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
    }

    #[test]
    fn non_left_up_does_not_terminate_left_caption_session() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        let right_up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Right),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );

        assert_eq!(right_up.caption_action, None);
        assert_eq!(
            pressed_caption_button.get(),
            Some(WindowsCaptionButtonAction::Close)
        );

        let left_up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        assert_eq!(
            left_up.caption_action,
            Some(WindowsCaptionButtonAction::Close)
        );
    }

    #[test]
    fn non_client_caption_down_then_client_left_up_terminates_without_native_release() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };
        let mut native_effects = Vec::new();

        let down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );
        assert!(down.consumed);

        boundary.pointer.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );

        assert!(native_effects.is_empty());
        assert_eq!(pressed_caption_button.get(), None);

        let later_non_client_up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |effect| {
                native_effects.push(effect);
                true
            },
        );
        assert_eq!(later_non_client_up.caption_action, None);
        assert!(native_effects.is_empty());
    }

    #[test]
    fn canceled_non_client_caption_button_cannot_commit_on_later_up() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|_| DispatchEventResult::default()));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        let down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        assert!(down.consumed);
        assert_eq!(down.caption_action, None);
        assert_eq!(
            pressed_caption_button.get(),
            Some(WindowsCaptionButtonAction::Close)
        );

        assert!(
            boundary
                .handle_pointer_cancel(PointerCancelReason::PlatformCaptureLost)
                .canceled
        );
        assert_eq!(pressed_caption_button.get(), None);

        let up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        assert!(!up.consumed);
        assert_eq!(up.caption_action, None);
    }

    #[test]
    fn consumed_non_client_caption_button_up_clears_pending_action() {
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new(|input| DispatchEventResult {
            propagate: !matches!(input, PlatformInput::MouseUp(_)),
            default_prevented: false,
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        let up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );

        assert!(up.consumed);
        assert_eq!(up.caption_action, None);
        assert_eq!(pressed_caption_button.get(), None);
    }

    #[test]
    fn reentrant_cancel_during_non_client_down_does_not_restore_caption_action() {
        let pointer_capture = Rc::new(Cell::new(WindowsPointerCaptureState::default()));
        let pressed_caption_button = Rc::new(Cell::new(None));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let pointer_capture = pointer_capture.clone();
            let pressed_caption_button = pressed_caption_button.clone();
            move |input| {
                if matches!(input, PlatformInput::MouseDown(_)) {
                    let (next_capture, _) =
                        pointer_capture
                            .get()
                            .transition(WindowsPointerCaptureInput::Cancel(
                                PointerCancelReason::PlatformCaptureLost,
                            ));
                    pointer_capture.set(next_capture);
                    pressed_caption_button.take();
                }
                DispatchEventResult::default()
            }
        }));
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: pointer_capture.as_ref(),
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: pressed_caption_button.as_ref(),
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        let down = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );

        assert!(down.consumed);
        assert_eq!(down.caption_action, None);
        assert_eq!(pressed_caption_button.get(), None);
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
    }

    #[test]
    fn reentrant_cancel_during_non_client_up_revokes_caption_action() {
        let pointer_capture = Rc::new(Cell::new(WindowsPointerCaptureState::default()));
        let pressed_caption_button = Rc::new(Cell::new(None));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let pointer_capture = pointer_capture.clone();
            let pressed_caption_button = pressed_caption_button.clone();
            move |input| {
                if matches!(input, PlatformInput::MouseUp(_)) {
                    let (next_capture, _) =
                        pointer_capture
                            .get()
                            .transition(WindowsPointerCaptureInput::Cancel(
                                PointerCancelReason::PlatformCaptureLost,
                            ));
                    pointer_capture.set(next_capture);
                    pressed_caption_button.take();
                }
                DispatchEventResult::default()
            }
        }));
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: pointer_capture.as_ref(),
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: pressed_caption_button.as_ref(),
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        let up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );

        assert_eq!(up.caption_action, None);
        assert_eq!(pressed_caption_button.get(), None);
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
    }

    #[test]
    fn window_deactivation_cancels_non_client_caption_session_exactly_once() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let observed = observed.clone();
            move |input| {
                if let PlatformInput::PointerCanceled(event) = input {
                    observed.borrow_mut().push(event.reason);
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsNonClientPointerBoundary {
            pointer: WindowsClientPointerBoundary {
                pointer_capture: &pointer_capture,
                native_pointer_capture_release: &Default::default(),
                pressed_caption_button: &pressed_caption_button,
                callbacks: &callbacks,
                dispatch_state: &dispatch_state,
            },
        };

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );

        let cancel = boundary.handle_pointer_cancel(PointerCancelReason::WindowDeactivated);
        assert!(cancel.canceled);
        assert!(!cancel.release_native_capture);
        let duplicate = boundary.handle_pointer_cancel(PointerCancelReason::WindowDeactivated);
        assert!(!duplicate.canceled);
        assert!(!duplicate.release_native_capture);
        assert_eq!(
            observed.borrow().as_slice(),
            &[PointerCancelReason::WindowDeactivated]
        );
        assert_eq!(pressed_caption_button.get(), None);

        let later_up = boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Up(MouseButton::Left),
            HTCLOSE,
            Point::default(),
            Modifiers::default(),
            1,
            |_| true,
        );
        assert_eq!(later_up.caption_action, None);
    }

    #[test]
    fn pointer_capture_cancel_is_terminal_and_deduplicated() {
        let (state, _) = WindowsPointerCaptureState::default().transition(
            WindowsPointerCaptureInput::ButtonDown {
                button: MouseButton::Left,
                acquire_native_capture: true,
            },
        );
        let (state, effect) = state.transition(WindowsPointerCaptureInput::Cancel(
            PointerCancelReason::PlatformCaptureLost,
        ));
        assert_eq!(state, WindowsPointerCaptureState::default());
        assert_eq!(
            effect,
            WindowsPointerCaptureEffect::Cancel(PointerCancelReason::PlatformCaptureLost)
        );

        let (_, duplicate_effect) = state.transition(WindowsPointerCaptureInput::Cancel(
            PointerCancelReason::PlatformCaptureLost,
        ));
        assert_eq!(duplicate_effect, WindowsPointerCaptureEffect::None);
    }

    #[test]
    fn input_dispatch_retains_only_the_first_pending_terminal_cancel() {
        let state = WindowsInputDispatchState::Dispatching {
            pending_terminal_cancel: None,
            terminal_cancel_reserved: false,
        }
        .defer_terminal_cancel(PointerCancelReason::PlatformCaptureLost)
        .defer_terminal_cancel(PointerCancelReason::WindowDeactivated);

        assert_eq!(
            state,
            WindowsInputDispatchState::Dispatching {
                pending_terminal_cancel: Some(PointerCancelReason::PlatformCaptureLost),
                terminal_cancel_reserved: false,
            }
        );

        let (_, panic_recovery) = state.into_panic_recovery().take_panic_recovery();
        assert!(should_reserve_pointer_cancel_after_callback_panic(
            false,
            panic_recovery,
        ));

        let (state, pending) = state.take_pending_terminal_cancel();
        assert_eq!(pending, Some(PointerCancelReason::PlatformCaptureLost));
        let (_, duplicate) = state.take_pending_terminal_cancel();
        assert_eq!(duplicate, None);
    }

    #[test]
    fn mouse_leave_is_hover_only_for_panic_recovery() {
        assert!(!may_own_pointer_session(WM_MOUSELEAVE, WPARAM::default()));
        assert!(!may_own_pointer_session(WM_NCMOUSELEAVE, WPARAM::default()));
        assert!(may_own_pointer_session(WM_MOUSEMOVE, WPARAM::default()));
    }

    #[test]
    fn uncaptured_hover_panic_does_not_reserve_pointer_cancel() {
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _dispatch_guard = WindowsInputDispatchGuard::begin(&dispatch_state, None);
            panic!("injected uncaptured hover callback panic");
        }));

        assert!(panic.is_err());
        let (state, panic_recovery) = dispatch_state.get().take_panic_recovery();
        dispatch_state.set(state);
        let panic_recovery = panic_recovery.expect("input panic recovery should be retained");
        assert!(!panic_recovery.terminal_cancel_reserved);
        assert_eq!(panic_recovery.pending_terminal_cancel, None);
        assert!(!should_reserve_pointer_cancel_after_callback_panic(
            WindowsPointerCaptureState::default().has_active_session(),
            Some(panic_recovery),
        ));
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn mouse_up_panic_releases_capture_without_reserving_duplicate_cancel() {
        let callback_count = Rc::new(Cell::new(0_usize));
        let callbacks = Callbacks::default();
        callbacks.set_test_input(Box::new({
            let callback_count = callback_count.clone();
            move |_| {
                let count = callback_count.get();
                callback_count.set(count + 1);
                if count == 1 {
                    panic!("injected mouse-up callback panic");
                }
                DispatchEventResult::default()
            }
        }));
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = Cell::new(None);
        let boundary = WindowsClientPointerBoundary {
            pointer_capture: &pointer_capture,
            native_pointer_capture_release: &Default::default(),
            pressed_caption_button: &pressed_caption_button,
            callbacks: &callbacks,
            dispatch_state: &dispatch_state,
        };
        let effects = Rc::new(RefCell::new(Vec::new()));

        boundary.handle_button_message(
            WindowsClientMouseButtonMessage::Down(MouseButton::Left),
            Point::default(),
            Modifiers::default(),
            1,
            {
                let effects = effects.clone();
                move |effect| {
                    effects.borrow_mut().push(effect);
                    true
                }
            },
        );
        let panic = catch_unwind(AssertUnwindSafe(|| {
            boundary.handle_button_message(
                WindowsClientMouseButtonMessage::Up(MouseButton::Left),
                Point::default(),
                Modifiers::default(),
                1,
                {
                    let effects = effects.clone();
                    move |effect| {
                        effects.borrow_mut().push(effect);
                        true
                    }
                },
            );
        }));

        assert!(panic.is_err());
        assert_eq!(
            effects.borrow().as_slice(),
            [
                WindowsPointerCaptureEffect::Acquire,
                WindowsPointerCaptureEffect::Release,
            ]
        );
        assert_eq!(pointer_capture.get(), WindowsPointerCaptureState::default());
        let (state, panic_recovery) = dispatch_state.get().take_panic_recovery();
        dispatch_state.set(state);
        let panic_recovery = panic_recovery.expect("input panic recovery should be retained");
        assert!(!panic_recovery.terminal_cancel_reserved);
        assert_eq!(panic_recovery.pending_terminal_cancel, None);
        assert!(!should_reserve_pointer_cancel_after_callback_panic(
            pointer_capture.get().has_active_session(),
            Some(panic_recovery),
        ));
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn reentrant_capture_loss_then_outer_panic_preserves_single_terminal_reservation() {
        let dispatch_state = Cell::new(WindowsInputDispatchState::default());
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _dispatch_guard = WindowsInputDispatchGuard::begin(&dispatch_state, None);
            dispatch_state.set(dispatch_state.get().reserve_terminal_cancel());
            panic!("injected outer callback panic after reentrant capture loss");
        }));

        assert!(panic.is_err());
        assert_eq!(
            dispatch_state.get(),
            WindowsInputDispatchState::RecoveringAfterPanic {
                pending_terminal_cancel: None,
                terminal_cancel_reserved: true,
            }
        );
        let (state, panic_recovery) = dispatch_state.get().take_panic_recovery();
        dispatch_state.set(state);
        let panic_recovery = panic_recovery.expect("input panic recovery should be retained");
        assert!(panic_recovery.terminal_cancel_reserved);
        assert_eq!(panic_recovery.pending_terminal_cancel, None);
        assert!(!should_reserve_pointer_cancel_after_callback_panic(
            true,
            Some(panic_recovery),
        ));
        assert_eq!(dispatch_state.get(), WindowsInputDispatchState::Idle);
    }

    #[test]
    fn immovable_windows_still_hit_test_caption_buttons() {
        assert_eq!(
            hit_test_window_control_area(WindowControlArea::Drag, false),
            None
        );
        assert_eq!(
            hit_test_window_control_area(WindowControlArea::Close, false),
            Some(HTCLOSE as _)
        );
        assert_eq!(
            hit_test_window_control_area(WindowControlArea::Max, false),
            Some(HTMAXBUTTON as _)
        );
        assert_eq!(
            hit_test_window_control_area(WindowControlArea::Min, false),
            Some(HTMINBUTTON as _)
        );
    }

    #[test]
    fn movable_windows_hit_test_caption_drag_area() {
        assert_eq!(
            hit_test_window_control_area(WindowControlArea::Drag, true),
            Some(HTCAPTION as _)
        );
    }
}
