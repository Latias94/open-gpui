#![cfg_attr(test, allow(dead_code))]

use crate::{RegisteredWindow, WindowsWindowInner};
use open_gpui::WindowId;
use parking_lot::Mutex;
use std::sync::{
    Arc, OnceLock, Weak,
    atomic::{AtomicU64, Ordering},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::GetCapture,
        WindowsAndMessaging::{
            GetMessageExtraInfo, WM_CANCELMODE, WM_CAPTURECHANGED, WM_CLOSE, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY,
        },
    },
};

static NEXT_OBSERVATION_TOKEN: AtomicU64 = AtomicU64::new(1);
static NEXT_OBSERVATION_ORDINAL: AtomicU64 = AtomicU64::new(1);
static ACTIVE_OBSERVATION: OnceLock<Mutex<Option<ActiveObservation>>> = OnceLock::new();

#[derive(Clone)]
struct ActiveObservation {
    token: u64,
    state: Weak<Mutex<NativeWindowTestObservationState>>,
}

#[derive(Default)]
struct NativeWindowTestObservationState {
    clear_generation: u64,
    events: Vec<NativeWindowTestEvent>,
}

/// Exact identity of one native window generation observed by the Windows test harness.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeWindowTestIdentity {
    window_id: WindowId,
    native_generation: usize,
}

impl NativeWindowTestIdentity {
    fn from_registration(registration: RegisteredWindow) -> Self {
        Self {
            window_id: registration.window_id(),
            native_generation: registration.generation(),
        }
    }

    /// Returns the GPUI window identity for this native generation.
    pub fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the process-unique native generation assigned when the HWND was created.
    pub fn native_generation(self) -> usize {
        self.native_generation
    }
}

/// Signed device-pixel point captured inside one WndProc callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowTestPoint {
    /// Horizontal coordinate in device pixels.
    pub x: i32,
    /// Vertical coordinate in device pixels.
    pub y: i32,
}

/// Native messages whose exact recipient matters to multi-window input and teardown tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowTestMessage {
    MouseMove,
    PrimaryButtonDown,
    PrimaryButtonUp,
    MouseLeave,
    CaptureChanged,
    CancelMode,
    Close,
    NonClientDestroy,
    Other(u32),
}

impl NativeWindowTestMessage {
    fn from_message(message: u32) -> Self {
        match message {
            WM_MOUSEMOVE => Self::MouseMove,
            WM_LBUTTONDOWN => Self::PrimaryButtonDown,
            WM_LBUTTONUP => Self::PrimaryButtonUp,
            WM_MOUSELEAVE => Self::MouseLeave,
            WM_CAPTURECHANGED => Self::CaptureChanged,
            WM_CANCELMODE => Self::CancelMode,
            WM_CLOSE => Self::Close,
            WM_NCDESTROY => Self::NonClientDestroy,
            message => Self::Other(message),
        }
    }

    fn carries_client_point(self) -> bool {
        matches!(
            self,
            Self::MouseMove | Self::PrimaryButtonDown | Self::PrimaryButtonUp
        )
    }
}

/// Capture ownership observed at a WndProc boundary without exposing a raw HWND.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowTestCaptureOwner {
    None,
    Recipient,
    Other,
}

/// Terminal disposition of one WndProc callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowTestMessageDisposition {
    Returned(isize),
    Panicked,
}

/// Typed event emitted by the Windows native test observation seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowTestEventKind {
    WindowMessage {
        message: NativeWindowTestMessage,
        extra_info: isize,
        client_point: Option<NativeWindowTestPoint>,
        screen_point: Option<NativeWindowTestPoint>,
        capture_before: NativeWindowTestCaptureOwner,
        capture_after: NativeWindowTestCaptureOwner,
        disposition: NativeWindowTestMessageDisposition,
    },
    PresentationQuiesced {
        ticket_generation: u64,
    },
    DestroyEntered {
        ticket_generation: u64,
    },
    NativeTerminal {
        ticket_generation: u64,
    },
}

/// One process-ordered native observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowTestEvent {
    ordinal: u64,
    window: NativeWindowTestIdentity,
    kind: NativeWindowTestEventKind,
}

impl NativeWindowTestEvent {
    /// Returns the process-wide observation order.
    pub fn ordinal(self) -> u64 {
        self.ordinal
    }

    /// Returns the exact native window generation that emitted the event.
    pub fn window(self) -> NativeWindowTestIdentity {
        self.window
    }

    /// Returns the typed native event payload.
    pub fn kind(self) -> NativeWindowTestEventKind {
        self.kind
    }
}

/// Read-only handle for the currently installed native observation session.
#[derive(Clone)]
pub struct NativeWindowTestObservation {
    state: Arc<Mutex<NativeWindowTestObservationState>>,
}

/// Stable suffix cursor for one native observation session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowTestObservationCursor {
    clear_generation: u64,
    index: usize,
    ordinal: u64,
}

impl NativeWindowTestObservation {
    /// Returns all observations ordered by their callback/lifecycle ordinal.
    pub fn events(&self) -> Vec<NativeWindowTestEvent> {
        let mut events = self.state.lock().events.clone();
        events.sort_unstable_by_key(|event| event.ordinal);
        events
    }

    /// Captures a stable cursor immediately after the observations currently recorded.
    pub fn cursor(&self) -> NativeWindowTestObservationCursor {
        let state = self.state.lock();
        NativeWindowTestObservationCursor {
            clear_generation: state.clear_generation,
            index: state.events.len(),
            ordinal: state
                .events
                .iter()
                .map(|event| event.ordinal)
                .max()
                .unwrap_or(0),
        }
    }

    /// Returns observations appended after `cursor`, ordered by callback/lifecycle ordinal.
    pub fn events_after(
        &self,
        cursor: NativeWindowTestObservationCursor,
    ) -> Vec<NativeWindowTestEvent> {
        let state = self.state.lock();
        let start = cursor_start(&state, cursor);
        let mut events = state.events[start..]
            .iter()
            .copied()
            .filter(|event| event.ordinal > cursor.ordinal)
            .collect::<Vec<_>>();
        events.sort_unstable_by_key(|event| event.ordinal);
        events
    }

    /// Returns whether any observation appended after `cursor` matches `predicate`.
    pub fn any_after(
        &self,
        cursor: NativeWindowTestObservationCursor,
        mut predicate: impl FnMut(NativeWindowTestEvent) -> bool,
    ) -> bool {
        let state = self.state.lock();
        let start = cursor_start(&state, cursor);
        state.events[start..]
            .iter()
            .copied()
            .any(|event| event.ordinal > cursor.ordinal && predicate(event))
    }

    /// Removes observations already consumed by the current scenario.
    pub fn clear(&self) {
        let mut state = self.state.lock();
        state.clear_generation = state
            .clear_generation
            .checked_add(1)
            .expect("native test observation clear generation space exhausted");
        state.events.clear();
    }
}

fn cursor_start(
    state: &NativeWindowTestObservationState,
    cursor: NativeWindowTestObservationCursor,
) -> usize {
    if state.clear_generation == cursor.clear_generation {
        cursor.index.min(state.events.len())
    } else {
        0
    }
}

/// Linear installation guard for the process-global native observation session.
pub struct NativeWindowTestObservationGuard {
    token: u64,
}

impl Drop for NativeWindowTestObservationGuard {
    fn drop(&mut self) {
        let mut active = active_observation().lock();
        if active
            .as_ref()
            .is_some_and(|active| active.token == self.token)
        {
            active.take();
        }
    }
}

/// Installs one process-global native observation session.
///
/// The returned guard must remain alive while the application is running. Native interactive
/// tests are intentionally serialized because they also share the system cursor and capture.
pub fn begin_native_window_test_observation() -> (
    NativeWindowTestObservationGuard,
    NativeWindowTestObservation,
) {
    let token = NEXT_OBSERVATION_TOKEN.fetch_add(1, Ordering::Relaxed);
    assert_ne!(token, 0, "native test observation token space exhausted");
    let state = Arc::new(Mutex::new(NativeWindowTestObservationState::default()));
    let mut active = active_observation().lock();
    if active
        .as_ref()
        .and_then(|active| active.state.upgrade())
        .is_some()
    {
        panic!("a native window test observation session is already active");
    }
    *active = Some(ActiveObservation {
        token,
        state: Arc::downgrade(&state),
    });
    drop(active);
    (
        NativeWindowTestObservationGuard { token },
        NativeWindowTestObservation { state },
    )
}

pub(crate) struct PendingNativeWindowMessageObservation {
    ordinal: u64,
    window: NativeWindowTestIdentity,
    recipient: HWND,
    message: NativeWindowTestMessage,
    extra_info: isize,
    client_point: Option<NativeWindowTestPoint>,
    screen_point: Option<NativeWindowTestPoint>,
    capture_before: NativeWindowTestCaptureOwner,
}

impl PendingNativeWindowMessageObservation {
    pub(crate) fn complete(self, disposition: NativeWindowTestMessageDisposition) {
        record_event(NativeWindowTestEvent {
            ordinal: self.ordinal,
            window: self.window,
            kind: NativeWindowTestEventKind::WindowMessage {
                message: self.message,
                extra_info: self.extra_info,
                client_point: self.client_point,
                screen_point: self.screen_point,
                capture_before: self.capture_before,
                capture_after: capture_owner(self.recipient),
                disposition,
            },
        });
    }
}

pub(crate) fn begin_window_message_observation(
    window: &WindowsWindowInner,
    hwnd: HWND,
    message: u32,
    lparam: LPARAM,
) -> Option<PendingNativeWindowMessageObservation> {
    observation_is_active().then(|| {
        let message = NativeWindowTestMessage::from_message(message);
        let (client_point, screen_point) = if message.carries_client_point() {
            callback_points(hwnd, lparam)
        } else {
            (None, None)
        };
        PendingNativeWindowMessageObservation {
            ordinal: next_ordinal(),
            window: NativeWindowTestIdentity::from_registration(window.registration),
            recipient: hwnd,
            message,
            extra_info: unsafe { GetMessageExtraInfo() }.0,
            client_point,
            screen_point,
            capture_before: capture_owner(hwnd),
        }
    })
}

pub(crate) fn record_presentation_quiesced(window: &WindowsWindowInner, ticket_generation: u64) {
    record_lifecycle(
        window,
        NativeWindowTestEventKind::PresentationQuiesced { ticket_generation },
    );
}

pub(crate) fn record_destroy_entered(window: &WindowsWindowInner, ticket_generation: u64) {
    record_lifecycle(
        window,
        NativeWindowTestEventKind::DestroyEntered { ticket_generation },
    );
}

pub(crate) fn record_native_terminal(window: &WindowsWindowInner, ticket_generation: u64) {
    record_lifecycle(
        window,
        NativeWindowTestEventKind::NativeTerminal { ticket_generation },
    );
}

fn record_lifecycle(window: &WindowsWindowInner, kind: NativeWindowTestEventKind) {
    if !observation_is_active() {
        return;
    }
    record_event(NativeWindowTestEvent {
        ordinal: next_ordinal(),
        window: NativeWindowTestIdentity::from_registration(window.registration),
        kind,
    });
}

fn record_event(event: NativeWindowTestEvent) {
    let state = active_observation()
        .lock()
        .as_ref()
        .and_then(|active| active.state.upgrade());
    if let Some(state) = state {
        state.lock().events.push(event);
    }
}

fn observation_is_active() -> bool {
    active_observation()
        .lock()
        .as_ref()
        .is_some_and(|active| active.state.strong_count() > 0)
}

fn active_observation() -> &'static Mutex<Option<ActiveObservation>> {
    ACTIVE_OBSERVATION.get_or_init(|| Mutex::new(None))
}

fn next_ordinal() -> u64 {
    let ordinal = NEXT_OBSERVATION_ORDINAL.fetch_add(1, Ordering::Relaxed);
    assert_ne!(
        ordinal, 0,
        "native test observation ordinal space exhausted"
    );
    ordinal
}

fn capture_owner(recipient: HWND) -> NativeWindowTestCaptureOwner {
    let capture = unsafe { GetCapture() };
    if capture == HWND::default() {
        NativeWindowTestCaptureOwner::None
    } else if capture == recipient {
        NativeWindowTestCaptureOwner::Recipient
    } else {
        NativeWindowTestCaptureOwner::Other
    }
}

fn callback_points(
    hwnd: HWND,
    lparam: LPARAM,
) -> (Option<NativeWindowTestPoint>, Option<NativeWindowTestPoint>) {
    let client = NativeWindowTestPoint {
        x: lparam.0 as i16 as i32,
        y: (lparam.0 >> 16) as i16 as i32,
    };
    let mut screen = POINT {
        x: client.x,
        y: client.y,
    };
    let screen =
        unsafe { ClientToScreen(hwnd, &mut screen).as_bool() }.then_some(NativeWindowTestPoint {
            x: screen.x,
            y: screen.y,
        });
    (Some(client), screen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn observation_cursor_scans_only_the_appended_suffix_and_survives_clear() {
        let observation = NativeWindowTestObservation {
            state: Arc::new(Mutex::new(NativeWindowTestObservationState::default())),
        };
        observation.state.lock().events.extend([event(1), event(2)]);
        let cursor = observation.cursor();
        observation.state.lock().events.push(event(3));

        let visited = Cell::new(0);
        assert!(observation.any_after(cursor, |event| {
            visited.set(visited.get() + 1);
            event.ordinal() == 3
        }));
        assert_eq!(visited.get(), 1);
        assert_eq!(
            observation
                .events_after(cursor)
                .into_iter()
                .map(NativeWindowTestEvent::ordinal)
                .collect::<Vec<_>>(),
            [3]
        );

        observation.clear();
        observation.state.lock().events.push(event(4));
        assert_eq!(
            observation
                .events_after(cursor)
                .into_iter()
                .map(NativeWindowTestEvent::ordinal)
                .collect::<Vec<_>>(),
            [4]
        );
    }

    fn event(ordinal: u64) -> NativeWindowTestEvent {
        NativeWindowTestEvent {
            ordinal,
            window: NativeWindowTestIdentity {
                window_id: WindowId::from(1),
                native_generation: 1,
            },
            kind: NativeWindowTestEventKind::PresentationQuiesced {
                ticket_generation: ordinal,
            },
        }
    }
}
