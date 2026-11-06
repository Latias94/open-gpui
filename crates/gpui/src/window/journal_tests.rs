use super::cached_paint_atlas_tests::{CandidateFocusCompletionRoot, RejectOnceAtlas};
use crate::{
    App, AppContext as _, AtlasTextureKind, PlatformInput, PlatformWindowPresentOutcome,
    TestAppContext, TestWindow, Window, WindowHandle,
    profiler::journal::{
        self, FRAME_DEADLINE, ForegroundEvent, ForegroundJournalEntry, IntervalBoundary,
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

fn open_cpu_window(
    cx: &mut TestAppContext,
) -> (WindowHandle<CandidateFocusCompletionRoot>, TestWindow) {
    cx.defer_next_window_frame_requests();
    let handle = cx.add_window(|window, cx| {
        window.set_sprite_atlas_for_test(Arc::new(RejectOnceAtlas::new()));
        CandidateFocusCompletionRoot {
            focus: cx.focus_handle(),
            tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
            remaining_requests: Rc::new(Cell::new(0)),
            next_request_id: Rc::new(Cell::new(0)),
            outcomes: Rc::new(RefCell::new(Vec::new())),
            subscriptions: Rc::new(RefCell::new(Vec::new())),
        }
    });
    let platform = cx.test_window(handle.into());
    cx.run_until_parked();
    (handle, platform)
}

fn noninvalidating_input(window: &mut Window, cx: &mut App) {
    let updates = window.invalidator.update_count();
    let generation = window.rendered_frame_revision();
    window.dispatch_event(PlatformInput::MouseUp(Default::default()), cx);
    assert_eq!(window.invalidator.update_count(), updates);
    assert_eq!(window.rendered_frame_revision(), generation);
}

fn has_idle(entries: &[ForegroundJournalEntry]) -> bool {
    entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
        )
    })
}

fn assert_failed_retries_keep_the_original_deadline(
    cx: &mut TestAppContext,
    outcome: PlatformWindowPresentOutcome,
) {
    let (journal, _guard) = journal::install_test_foreground_journal(256, 8);
    let (handle, platform) = open_cpu_window(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        let mut collector = journal.collector();
        window.refresh();
        window.draw(cx).clear(cx);
        let generation = window.rendered_frame_revision();
        assert!(!window.invalidator.is_dirty());
        assert!(window.invalidator.pending_dirty_at().is_none());
        let platform_draws = platform.draw_count();
        platform.set_present_outcome(outcome);

        for _ in 0..2 {
            assert_eq!(window.present(cx), outcome);
            assert_eq!(window.rendered_frame_revision(), generation);
            assert!(window.needs_present.get());
        }
        let before_deadline = collector.collect_unseen();
        assert_eq!(before_deadline.lost, 0);
        assert!(!has_idle(&before_deadline.entries));

        std::thread::sleep(FRAME_DEADLINE + Duration::from_millis(10));
        assert_eq!(window.present(cx), outcome);
        let expired = collector.collect_unseen();
        assert_eq!(expired.lost, 0);
        assert!(
            has_idle(&expired.entries),
            "retries of one CPU TestPlatform scene must not renew its frame deadline"
        );
        assert!(expired.entries.iter().all(|entry| {
            !matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
                    | ForegroundJournalEntry::FrameState(_)
            )
        }));

        for _ in 0..2 {
            assert_eq!(window.present(cx), outcome);
            noninvalidating_input(window, cx);
            let idle = collector.collect_unseen();
            assert_eq!(idle.lost, 0);
            assert!(has_idle(&idle.entries));
        }
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(platform.draw_count(), platform_draws + 5);
        assert!(!window.invalidator.is_dirty());
        assert!(window.needs_present.get());
        platform.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
    })
    .unwrap();
}

#[open_gpui::test]
fn deferred_cpu_scene_retries_do_not_renew_the_frame_deadline(cx: &mut TestAppContext) {
    assert_failed_retries_keep_the_original_deadline(cx, PlatformWindowPresentOutcome::Deferred);
}

#[open_gpui::test]
fn rejected_cpu_scene_retries_do_not_renew_the_frame_deadline(cx: &mut TestAppContext) {
    assert_failed_retries_keep_the_original_deadline(cx, PlatformWindowPresentOutcome::Rejected);
}

#[open_gpui::test]
fn submitted_old_cpu_scene_keeps_newer_dirty_demand(cx: &mut TestAppContext) {
    let (journal, _guard) = journal::install_test_foreground_journal(256, 8);
    let (handle, _) = open_cpu_window(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        let generation = window.rendered_frame_revision();
        window.refresh();
        let dirty_at = window.invalidator.pending_dirty_at();
        assert!(dirty_at.is_some());
        let mut collector = journal.collector();

        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert!(window.invalidator.is_dirty());
        assert!(!window.needs_present.get());
        assert_eq!(window.invalidator.pending_dirty_at(), dirty_at);
        noninvalidating_input(window, cx);
        let pending = collector.collect_unseen();
        assert_eq!(pending.lost, 0);
        assert!(!has_idle(&pending.entries));
        assert!(pending.entries.iter().all(|entry| !matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        noninvalidating_input(window, cx);
        let submitted = collector.collect_unseen();
        assert_eq!(submitted.lost, 0);
        assert!(has_idle(&submitted.entries));
        assert!(submitted.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(frame))
                if frame.frame.window_id == handle.window_id()
        )));
    })
    .unwrap();
}

fn focus_cancellation_reaches_idle(
    cx: &mut TestAppContext,
    outcome: PlatformWindowPresentOutcome,
    unrelated_dirty: bool,
) -> bool {
    let (journal, _guard) = journal::install_test_foreground_journal(256, 8);
    let (handle, platform) = open_cpu_window(cx);
    let committed_focus = handle.update(cx, |root, _, _| root.focus.clone()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        let transient_focus = cx.focus_handle();
        let _observer = window.on_focus_committed(&committed_focus, cx, move |window, cx| {
            transient_focus.focus(window, cx);
        });
        committed_focus.focus(window, cx);
        window.draw(cx).clear(cx);
        let generation = window.rendered_frame_revision();
        assert!(window.invalidator.is_focus_only_dirty());
        assert!(window.pending_focus_claim.is_some());
        platform.set_present_outcome(outcome);
        assert_eq!(window.present(cx), outcome);
        let mut collector = journal.collector();

        if unrelated_dirty {
            window.refresh();
        }
        committed_focus.focus(window, cx);
        assert!(window.pending_focus_claim.is_none());
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.invalidator.is_dirty(), unrelated_dirty);
        assert_eq!(
            window.needs_present.get(),
            outcome != PlatformWindowPresentOutcome::Submitted
        );
        noninvalidating_input(window, cx);
        assert_eq!(window.rendered_frame_revision(), generation);
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(drained.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Input(input))
                if input.kind == "mouse_up" && !input.caused_invalidation
        )));
        assert!(drained.entries.iter().all(|entry| !matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
                | ForegroundJournalEntry::FrameState(journal::FrameStateChange::Closed { .. })
        )));
        let idle = has_idle(&drained.entries);
        platform.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        window.draw(cx).clear(cx);
        assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
        idle
    })
    .unwrap()
}

#[open_gpui::test]
fn canceled_cpu_focus_followup_releases_idle_without_another_draw(cx: &mut TestAppContext) {
    assert!(focus_cancellation_reaches_idle(
        cx,
        PlatformWindowPresentOutcome::Submitted,
        false,
    ));
}

#[open_gpui::test]
fn canceled_cpu_focus_followup_keeps_unsubmitted_scene_demand(cx: &mut TestAppContext) {
    assert!(!focus_cancellation_reaches_idle(
        cx,
        PlatformWindowPresentOutcome::Deferred,
        false,
    ));
}

#[open_gpui::test]
fn canceled_cpu_focus_followup_keeps_unrelated_dirty_demand(cx: &mut TestAppContext) {
    assert!(!focus_cancellation_reaches_idle(
        cx,
        PlatformWindowPresentOutcome::Submitted,
        true,
    ));
}
