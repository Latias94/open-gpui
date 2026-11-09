#![cfg(feature = "profiler")]

//! Ordinary public consumers without GPUI test-support.
//! Mode values execute directly; the explicitly unexecuted Window closure only checks API types.
//! This target does not construct an App or Window or qualify rendering or native submission.

use open_gpui::{DebugFrameOverlayMode, Window};

#[test]
fn downstream_mode_values_default_and_cycle_without_test_support() {
    let hidden: DebugFrameOverlayMode = Default::default();
    assert_eq!(hidden, DebugFrameOverlayMode::Hidden);
    assert_eq!(hidden.next(), DebugFrameOverlayMode::Minimal);
    assert_eq!(hidden.next().next(), DebugFrameOverlayMode::Full);
    assert_eq!(hidden.next().next().next(), DebugFrameOverlayMode::Hidden);
    let copied = hidden;
    assert_eq!(copied, hidden);
}

#[test]
fn downstream_window_signatures_type_check_in_an_unexecuted_closure() {
    let _: fn(&Window) -> DebugFrameOverlayMode = Window::debug_frame_overlay_mode;
    let _: fn(&mut Window, DebugFrameOverlayMode) = Window::set_debug_frame_overlay_mode;
    let _: fn(&mut Window) = Window::cycle_debug_frame_overlay_mode;
    let _: fn(&mut Window) = Window::reset_debug_frame_overlay_stats;
    let _: fn(DebugFrameOverlayMode) -> DebugFrameOverlayMode = DebugFrameOverlayMode::next;

    let _unexecuted_window_consumer = |window: &mut Window| {
        let previous: DebugFrameOverlayMode = window.debug_frame_overlay_mode();
        window.set_debug_frame_overlay_mode(previous.next());
        window.cycle_debug_frame_overlay_mode();
        window.reset_debug_frame_overlay_stats();
        let current: DebugFrameOverlayMode = window.debug_frame_overlay_mode();
        (previous, current)
    };
}
