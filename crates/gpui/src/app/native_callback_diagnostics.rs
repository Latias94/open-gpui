use crate::{DispatchEventResult, NativeCapturedDragGeneration, WindowId, WindowMutationDomain};

#[cfg(any(test, feature = "test-support"))]
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

#[cfg(any(test, feature = "test-support"))]
use parking_lot::Mutex;

#[cfg(any(test, feature = "test-support"))]
const TERMINAL_DIAGNOSTIC_CAPACITY: usize = 512;

/// A payload-free diagnostic emitted at a native callback or command boundary.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeBoundaryDiagnostic {
    /// The global native-ingress sequence assigned when the work entered GPUI.
    pub sequence: u64,
    /// The application or generational window targeted by the work.
    pub target: NativeBoundaryTarget,
    /// The exact callback or platform command that entered the boundary.
    pub kind: NativeBoundaryKind,
    /// A domain-specific generation when the callback belongs to a replaceable authority.
    pub domain_generation: Option<NativeBoundaryGeneration>,
    /// The current queue or terminal disposition.
    pub disposition: NativeBoundaryDisposition,
}

impl NativeBoundaryDiagnostic {
    pub(crate) fn pending(
        sequence: u64,
        target: NativeBoundaryTarget,
        kind: NativeBoundaryKind,
        domain_generation: Option<NativeBoundaryGeneration>,
    ) -> Self {
        Self {
            sequence,
            target,
            kind,
            domain_generation,
            disposition: NativeBoundaryDisposition::Pending,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn terminal(mut self, disposition: NativeBoundaryDisposition) -> Self {
        debug_assert_ne!(disposition, NativeBoundaryDisposition::Pending);
        self.disposition = disposition;
        self
    }
}

/// The target authority of native work.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum NativeBoundaryTarget {
    /// Application-wide native work.
    Application,
    /// Work for one generational GPUI window.
    Window(WindowId),
}

/// The native boundary operation represented by a diagnostic.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBoundaryKind {
    /// A callback initiated by the platform.
    Callback(NativeCallbackKind),
    /// A command delegated to the platform after releasing the app borrow.
    Command(NativePlatformCommandKind),
}

/// A payload-free native callback kind.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCallbackKind {
    OpenUrls,
    Reopen,
    SystemWake,
    KeyboardLayoutChanged,
    ThermalStateChanged,
    Quit,
    WillOpenAppMenu,
    AppMenuAction,
    AccessibilityActivated,
    AccessibilityDeactivated,
    AccessibilityAction,
    InitialPresentationCompleted,
    InitialPresentationFailed,
    RequestFrame,
    ActiveChanged,
    ModifiersChanged,
    HoverChanged,
    Resized,
    Moved,
    WindowStateChanged,
    WindowMutationObserved,
    ShouldClose,
    Closed,
    AppearanceChanged,
    ButtonLayoutChanged,
    SystemTabMergeAll,
    SystemTabMoveToNewWindow,
    SystemTabSelectNext,
    SystemTabSelectPrevious,
    SystemTabToggleBar,
    PlatformInput,
    CapturedDragCancellation,
    PlatformInputHandlerSlot,
    PlatformInputHandler(NativeInputHandlerOperation),
}

/// A payload-free platform input-handler operation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeInputHandlerOperation {
    SelectedTextRange,
    MarkedTextRange,
    TextForRange,
    ReplaceTextInRange,
    ReplaceAndMarkTextInRange,
    UnmarkText,
    BoundsForRange,
    ApplePressAndHoldEnabled,
    ImeCandidateBounds,
    CharacterIndexForPoint,
    AcceptsTextInput,
    PrefersImeForPrintableKeys,
}

/// A payload-free platform command kind.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlatformCommandKind {
    CompleteInitialPresentation,
    RevealDeferredInitialPresentation,
    Activate,
    ShowWindowMenu,
    StartWindowMove,
    StartWindowResize,
    ReleasePointerCapture,
    CompleteCapturedDragRelease,
    RetireNativeWindow,
    CompleteShutdown,
}

/// The replaceable authority generation attached to native work.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBoundaryGeneration {
    AppShutdown(u64),
    AccessibilityActivation(u64),
    CapturedDrag(NativeCapturedDragGeneration),
    PointerCaptureRelease {
        captured_drag: Option<NativeCapturedDragGeneration>,
        release: u64,
    },
    WindowMutation {
        domain: WindowMutationDomain,
        generation: u64,
    },
    ProvisionalPresentation {
        session_generation: u64,
        presentation_generation: u64,
    },
    PresentationShutdown(u64),
    InputSlot {
        boundary: NativeInputBoundary,
        generation: u64,
    },
}

/// The must-immediate input boundary whose slot generation was observed.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeInputBoundary {
    PlatformInput,
    InputHandler,
}

/// A queue or terminal native-boundary disposition.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBoundaryDisposition {
    Pending,
    Delivered {
        input_result: Option<NativeInputDeliveryResult>,
    },
    Coalesced {
        into_sequence: u64,
    },
    Rejected,
    Stale,
    Closed,
    InvariantFailure(NativeInvariantFailure),
}

/// A payload-free reason for rejecting a native boundary invariant.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeInvariantFailure {
    MissingLease,
    MissingSlot,
    RetiredSlot,
    SlotReentry,
    StaleWindow,
    ApplicationQuitting,
    AppBorrowBusy,
    ReservedWindow,
    EventTransactionReentry,
    BarrierBudgetExhausted,
    CallbackPanicked,
}

impl NativeBoundaryDisposition {
    pub(crate) const DELIVERED: Self = Self::Delivered { input_result: None };

    pub(crate) fn delivered_input(result: DispatchEventResult) -> Self {
        Self::Delivered {
            input_result: Some(NativeInputDeliveryResult {
                propagate: result.propagate,
                default_prevented: result.default_prevented,
            }),
        }
    }
}

/// The payload-free result returned to a native input dispatcher.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeInputDeliveryResult {
    pub propagate: bool,
    pub default_prevented: bool,
}

/// A monotonic cursor into the bounded terminal diagnostic stream.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeBoundaryDiagnosticCursor(u64);

/// A test-support view of current pending work and terminal diagnostics since a cursor.
#[doc(hidden)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeBoundaryDiagnosticsSnapshot {
    pub cursor: NativeBoundaryDiagnosticCursor,
    pub evicted_terminal_count: u64,
    pub omitted_before_cursor: u64,
    pub pending: Vec<NativeBoundaryDiagnostic>,
    pub terminal: Vec<NativeBoundaryDiagnostic>,
}

#[derive(Clone, Default)]
pub(crate) struct NativeBoundaryDiagnostics {
    #[cfg(any(test, feature = "test-support"))]
    state: Arc<Mutex<NativeBoundaryDiagnosticsState>>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
struct NativeBoundaryDiagnosticsState {
    next_cursor: u64,
    evicted_terminal_count: u64,
    terminal: VecDeque<(NativeBoundaryDiagnosticCursor, NativeBoundaryDiagnostic)>,
    terminal_sequences: HashSet<u64>,
    closed_windows: HashSet<WindowId>,
}

impl NativeBoundaryDiagnostics {
    pub(crate) fn record_terminal(
        &self,
        pending: NativeBoundaryDiagnostic,
        disposition: NativeBoundaryDisposition,
    ) {
        #[cfg(any(test, feature = "test-support"))]
        {
            let mut state = self.state.lock();
            assert!(
                state.terminal_sequences.insert(pending.sequence),
                "native boundary sequence {} reached more than one terminal disposition",
                pending.sequence
            );
            let disposition = match (pending.target, disposition) {
                (NativeBoundaryTarget::Window(window_id), NativeBoundaryDisposition::Stale)
                    if state.closed_windows.contains(&window_id) =>
                {
                    NativeBoundaryDisposition::Closed
                }
                _ => disposition,
            };
            if matches!(
                pending.kind,
                NativeBoundaryKind::Callback(NativeCallbackKind::Closed)
            ) && disposition == NativeBoundaryDisposition::DELIVERED
                && let NativeBoundaryTarget::Window(window_id) = pending.target
            {
                state.closed_windows.insert(window_id);
            }
            let diagnostic = pending.terminal(disposition);
            state.next_cursor = state
                .next_cursor
                .checked_add(1)
                .expect("native boundary diagnostic cursor overflowed");
            let cursor = NativeBoundaryDiagnosticCursor(state.next_cursor);
            if state.terminal.len() == TERMINAL_DIAGNOSTIC_CAPACITY {
                if let Some((_, evicted)) = state.terminal.pop_front() {
                    state.terminal_sequences.remove(&evicted.sequence);
                }
                state.evicted_terminal_count = state.evicted_terminal_count.saturating_add(1);
            }
            state.terminal.push_back((cursor, diagnostic));
        }
        #[cfg(not(any(test, feature = "test-support")))]
        {
            let _ = (pending, disposition);
        }
    }

    pub(crate) fn reopen_window(&self, window_id: WindowId) {
        #[cfg(any(test, feature = "test-support"))]
        self.state.lock().closed_windows.remove(&window_id);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = window_id;
    }

    pub(crate) fn close_window(&self, window_id: WindowId) {
        #[cfg(any(test, feature = "test-support"))]
        self.state.lock().closed_windows.insert(window_id);
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = window_id;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn snapshot_since(
        &self,
        cursor: NativeBoundaryDiagnosticCursor,
        mut pending: Vec<NativeBoundaryDiagnostic>,
    ) -> NativeBoundaryDiagnosticsSnapshot {
        pending.sort_by_key(|diagnostic| diagnostic.sequence);
        let state = self.state.lock();
        let omitted_before_cursor = state
            .terminal
            .front()
            .map(|(oldest_cursor, _)| oldest_cursor.0.saturating_sub(cursor.0.saturating_add(1)))
            .unwrap_or(0);
        NativeBoundaryDiagnosticsSnapshot {
            cursor: NativeBoundaryDiagnosticCursor(state.next_cursor),
            evicted_terminal_count: state.evicted_terminal_count,
            omitted_before_cursor,
            pending,
            terminal: state
                .terminal
                .iter()
                .filter_map(|(entry_cursor, diagnostic)| {
                    (*entry_cursor > cursor).then_some(*diagnostic)
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(sequence: u64) -> NativeBoundaryDiagnostic {
        NativeBoundaryDiagnostic::pending(
            sequence,
            NativeBoundaryTarget::Application,
            NativeBoundaryKind::Callback(NativeCallbackKind::SystemWake),
            None,
        )
    }

    #[test]
    fn terminal_ring_is_bounded_and_cursor_reads_only_new_entries() {
        let diagnostics = NativeBoundaryDiagnostics::default();
        for sequence in 0..(TERMINAL_DIAGNOSTIC_CAPACITY as u64 + 2) {
            diagnostics.record_terminal(pending(sequence), NativeBoundaryDisposition::DELIVERED);
        }

        let snapshot =
            diagnostics.snapshot_since(NativeBoundaryDiagnosticCursor::default(), Vec::new());
        assert_eq!(snapshot.terminal.len(), TERMINAL_DIAGNOSTIC_CAPACITY);
        assert_eq!(snapshot.evicted_terminal_count, 2);
        assert_eq!(snapshot.omitted_before_cursor, 2);
        assert_eq!(snapshot.terminal[0].sequence, 2);

        let cursor = snapshot.cursor;
        diagnostics.record_terminal(
            pending(999),
            NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::SlotReentry),
        );
        let delta = diagnostics.snapshot_since(cursor, Vec::new());
        assert_eq!(delta.terminal.len(), 1);
        assert_eq!(delta.terminal[0].sequence, 999);
    }

    #[test]
    fn closed_window_barrier_distinguishes_teardown_from_stale_generation() {
        let diagnostics = NativeBoundaryDiagnostics::default();
        let window_id = WindowId::from(7);
        let window_pending = |sequence| {
            NativeBoundaryDiagnostic::pending(
                sequence,
                NativeBoundaryTarget::Window(window_id),
                NativeBoundaryKind::Callback(NativeCallbackKind::Moved),
                None,
            )
        };

        diagnostics.close_window(window_id);
        diagnostics.record_terminal(window_pending(0), NativeBoundaryDisposition::Stale);
        diagnostics.reopen_window(window_id);
        diagnostics.record_terminal(window_pending(1), NativeBoundaryDisposition::Stale);

        let snapshot =
            diagnostics.snapshot_since(NativeBoundaryDiagnosticCursor::default(), Vec::new());
        assert_eq!(
            snapshot.terminal[0].disposition,
            NativeBoundaryDisposition::Closed
        );
        assert_eq!(
            snapshot.terminal[1].disposition,
            NativeBoundaryDisposition::Stale
        );
    }

    #[test]
    #[should_panic(expected = "reached more than one terminal disposition")]
    fn one_native_sequence_cannot_publish_two_terminal_diagnostics() {
        let diagnostics = NativeBoundaryDiagnostics::default();
        diagnostics.record_terminal(pending(7), NativeBoundaryDisposition::DELIVERED);
        diagnostics.record_terminal(
            pending(7),
            NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::CallbackPanicked),
        );
    }
}
