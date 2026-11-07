use super::*;
use journal::{ForegroundEvent, ForegroundJournalEntry, IntervalBoundary};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

fn profiler() -> WindowProfiler {
    WindowProfiler::new(WindowId::from(0xF3A0)).expect("valid histograms")
}

fn frame_at(
    base: Instant,
    refresh_interval: Option<Duration>,
    signal_ms: Option<u64>,
) -> FrameTiming {
    FrameTiming {
        window_id: WindowId::from(0xF3A0),
        dirty_at: Some(base),
        invalidations: 1,
        draw_start: base,
        draw_end: base + Duration::from_millis(1),
        refresh_interval,
        signal_at: signal_ms.map(|ms| base + Duration::from_millis(ms)),
        phases: DrawPhases::default(),
    }
}

fn input_at(profiler: &mut WindowProfiler, start: Instant) {
    profiler.begin_input("KeyDown");
    let Some(WindowActivity::Input { started_at, .. }) = profiler.active_activities.last_mut()
    else {
        panic!("input must be active");
    };
    *started_at = start;
}

fn begin_view_at(profiler: &mut WindowProfiler, start: Instant) {
    profiler.begin_view();
    profiler.active_views.last_mut().unwrap().start = start;
}

fn views(entries: &[ForegroundJournalEntry]) -> Vec<ViewTiming> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::Event(ForegroundEvent::View(timing)) => Some(*timing),
            _ => None,
        })
        .collect()
}

#[test]
fn frame_work_starts_at_the_later_of_signal_and_invalidation() {
    let refresh = Duration::from_millis(10);
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let frame_with = |dirty: Option<u64>, signal: Option<u64>| FrameTiming {
        dirty_at: dirty.map(at),
        draw_start: at(1),
        draw_end: at(2),
        ..frame_at(start, Some(refresh), signal)
    };
    let work = |dirty, signal| frame_with(dirty, signal).work(at(50));

    assert_eq!(work(Some(0), Some(8)), Some(Duration::from_millis(42)));
    assert_eq!(work(Some(0), Some(40)), Some(Duration::from_millis(10)));
    assert_eq!(work(Some(5), Some(0)), Some(Duration::from_millis(45)));
    assert_eq!(work(None, Some(0)), Some(Duration::from_millis(50)));
    assert_eq!(work(Some(0), None), None);
    assert_eq!(work(None, None), None);

    let frame = frame_with(Some(0), Some(0));
    for (submitted_ms, missed) in [(0, 0), (7, 0), (10, 0), (20, 1), (25, 2)] {
        assert_eq!(frame.missed_refreshes(at(submitted_ms)), Some(missed));
    }
    assert_eq!(frame.draw_duration(), Duration::from_millis(1));
    assert_eq!(
        frame.dirty_to_draw_duration(),
        Some(Duration::from_millis(2))
    );
    assert_eq!(frame_with(None, Some(0)).dirty_to_draw_duration(), None);
    assert_eq!(frame_with(None, None).is_late(at(50)), None);
    assert_eq!(
        frame_with(Some(5), Some(8)).work(at(2)),
        Some(Duration::ZERO)
    );
}

#[test]
fn frames_are_split_by_refresh_interval_and_classified() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let sixty_hertz = Duration::from_secs(1) / 60;
    let one_hundred_twenty_hertz = Duration::from_secs(1) / 120;
    let three_hundred_sixty_hertz = Duration::from_secs(1) / 360;
    let base = Instant::now();
    let at = |ms| base + Duration::from_millis(ms);

    for (refresh_interval, signal_ms, present_start_ms, responded_to_input) in [
        (Some(sixty_hertz), Some(1), 5, true),
        (Some(sixty_hertz), Some(0), 40, true),
        (Some(sixty_hertz), None, 5, false),
        (Some(sixty_hertz), Some(40), 42, false),
        (Some(one_hundred_twenty_hertz), Some(0), 12, false),
        (Some(three_hundred_sixty_hertz), Some(0), 6, false),
        (Some(three_hundred_sixty_hertz), Some(0), 9, false),
        (None, Some(0), 2, false),
    ] {
        profiler.set_refresh_interval(refresh_interval);
        if responded_to_input {
            input_at(&mut profiler, at(0));
            profiler.end_input(true);
        }
        profiler.record_draw_timing(frame_at(base, refresh_interval, signal_ms));
        profiler.record_present(at(present_start_ms), at(present_start_ms + 1), true, false);
    }

    let snapshot = profiler.frame_duration_snapshot();
    let frames_for = |refresh_interval| {
        snapshot
            .by_refresh_interval
            .iter()
            .find(|frames| frames.refresh_interval == refresh_interval)
            .cloned()
            .expect("frames for the refresh interval")
    };
    let work_buckets = |work: &Histogram<u64>, refresh_interval: Duration| {
        histogram_bucket_counts(
            work,
            FRAME_WORK_BUCKETS_PERCENT
                .map(|percent| (refresh_interval * percent as u32 / 100).as_nanos() as u64),
        )
    };
    let sixty = frames_for(Some(sixty_hertz));
    assert_eq!(sixty.frames_drawn, 4);
    assert_eq!(sixty.frames_on_time, 2);
    assert_eq!(sixty.frames_late, 1);
    assert_eq!(sixty.missed_refreshes, 2);
    assert_eq!(sixty.frames_unmeasured, 1);
    assert_eq!(
        work_buckets(&sixty.work, sixty_hertz),
        [2, 0, 0, 0, 0, 0, 1, 0]
    );
    assert_eq!(sixty.input_frames_on_time, 1);
    assert_eq!(sixty.input_frames_late, 1);
    assert_eq!(
        work_buckets(&sixty.input_work, sixty_hertz),
        [1, 0, 0, 0, 0, 0, 1, 0]
    );

    let one_hundred_twenty = frames_for(Some(one_hundred_twenty_hertz));
    assert_eq!(one_hundred_twenty.frames_late, 1);
    assert_eq!(one_hundred_twenty.missed_refreshes, 1);
    assert_eq!(one_hundred_twenty.input_frames_late, 0);

    let three_hundred_sixty = frames_for(Some(three_hundred_sixty_hertz));
    assert_eq!(three_hundred_sixty.frames_on_time, 1);
    assert_eq!(three_hundred_sixty.frames_late, 1);
    assert_eq!(three_hundred_sixty.missed_refreshes, 5);

    let unknown = frames_for(None);
    assert_eq!(unknown.frames_drawn, 1);
    assert_eq!(unknown.frames_on_time, 1);
    assert_eq!(unknown.missed_refreshes, 0);
    assert_eq!(snapshot.draw_duration_histogram.len(), 8);

    let later = profiler.frame_duration_snapshot();
    let delta = later.by_refresh_interval[0].since(&snapshot.by_refresh_interval[0]);
    assert_eq!(
        delta.frames_drawn + delta.frames_on_time + delta.frames_late,
        0
    );
    assert_eq!(
        delta.draw_duration.len() + delta.work.len() + delta.input_work.len(),
        0
    );
}

#[test]
fn budgets_use_the_floor_and_unknown_fallback_but_misses_use_the_real_interval() {
    let base = Instant::now();
    for refresh_interval in [
        Some(Duration::from_millis(4)),
        Some(FRAME_BUDGET_FLOOR),
        Some(Duration::from_millis(33)),
        None,
    ] {
        let frame = frame_at(base, refresh_interval, Some(0));
        let budget = refresh_interval
            .unwrap_or(UNKNOWN_REFRESH_FRAME_BUDGET)
            .max(FRAME_BUDGET_FLOOR);
        assert_eq!(frame.budget(), budget);
        assert_eq!(frame.is_late(base + budget), Some(false));
        assert_eq!(
            frame.is_late(base + budget + Duration::from_nanos(1)),
            Some(true)
        );
    }
    let frame = frame_at(base, Some(Duration::from_millis(4)), Some(0));
    assert_eq!(frame.is_late(base + Duration::from_millis(8)), Some(false));
    assert_eq!(
        frame.missed_refreshes(base + Duration::from_millis(8)),
        Some(1)
    );
    assert_eq!(frame_at(base, None, Some(0)).missed_refreshes(base), None);
    assert_eq!(
        frame_at(base, Some(Duration::ZERO), Some(0)).missed_refreshes(base),
        None
    );
    assert_eq!(
        frame_at(base, Some(Duration::ZERO), Some(0)).budget(),
        UNKNOWN_REFRESH_FRAME_BUDGET
    );
}

#[test]
fn histogram_buckets_count_each_sample_once_including_shared_bounds_and_overflow() {
    let mut histogram = Histogram::<u64>::new(3).unwrap();
    for sample in [0, 1, 1, 2, 5, 8] {
        histogram.record(sample).unwrap();
    }
    assert_eq!(
        histogram_bucket_counts(&histogram, [0, 1, 1, 2, 4, 5]),
        [1, 2, 0, 1, 0, 1, 1]
    );
    assert_eq!(histogram_bucket_counts(&histogram, []), [6]);
    assert_eq!(histogram_bucket_counts(&histogram, [u64::MAX]), [6, 0]);

    let mut coarse = Histogram::<u64>::new(1).unwrap();
    coarse.record(10_000).unwrap();
    let counts = histogram_bucket_counts(&coarse, [10_000, 10_001, 10_002]);
    assert_eq!(counts.iter().sum::<u64>(), 1);
    assert!(counts.iter().all(|count| *count <= 1));
    assert_eq!(
        FRAME_DURATION_BUCKETS_MS,
        [4, 8, 16, 33, 50, 100, 250, 500, 1000]
    );
}

#[test]
fn refresh_snapshot_deltas_preserve_all_counts_and_histograms() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    let refresh = Some(Duration::from_millis(10));
    profiler.record_draw_timing(frame_at(base, refresh, Some(0)));
    profiler.record_present(
        base + Duration::from_millis(5),
        base + Duration::from_millis(6),
        true,
        false,
    );
    let earlier = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);

    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.record_draw_timing(frame_at(base, refresh, Some(0)));
    profiler.record_present(
        base + Duration::from_millis(25),
        base + Duration::from_millis(30),
        true,
        false,
    );
    profiler.record_draw_timing(frame_at(base, refresh, None));
    profiler.record_present(
        base + Duration::from_millis(40),
        base + Duration::from_millis(41),
        true,
        false,
    );
    let later = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);
    let delta = later.since(&earlier);
    assert_eq!(delta.refresh_interval, refresh);
    assert_eq!(delta.frames_drawn, 2);
    assert_eq!(delta.draw_duration.len(), 2);
    assert_eq!(delta.frames_on_time, 0);
    assert_eq!(delta.frames_late, 1);
    assert_eq!(delta.missed_refreshes, 2);
    assert_eq!(delta.frames_unmeasured, 1);
    assert_eq!(delta.work.len(), 1);
    assert_eq!(delta.input_frames_on_time, 0);
    assert_eq!(delta.input_frames_late, 1);
    assert_eq!(delta.input_work.len(), 1);
    assert_eq!(earlier.frames_drawn, 1);
    assert_eq!(earlier.work.len(), 1);

    let reversed = earlier.since(&later);
    assert_eq!(reversed.frames_drawn, 0);
    assert_eq!(reversed.frames_late, 0);
    assert_eq!(reversed.frames_unmeasured, 0);
    assert_eq!(reversed.work.len(), earlier.work.len());
}

#[test]
fn callback_changes_and_failed_retries_preserve_the_exact_accepted_metadata() {
    let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    let refresh = Some(Duration::from_millis(10));
    let phases = DrawPhases {
        request_layout: Duration::from_millis(1),
        prepaint: Duration::from_millis(2),
        paint: Duration::from_millis(3),
    };
    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.set_refresh_interval(refresh);
    profiler.record_frame_signal(Some(base));
    profiler.begin_draw();
    profiler.record_draw_phases(phases);
    profiler.end_draw(Some(base), 3);
    let accepted = profiler.pending_frame.unwrap().timing;

    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::RepaintRequired,
        PlatformWindowPresentOutcome::Rejected,
    ] {
        profiler.record_frame_signal(Some(base + Duration::from_millis(20)));
        profiler.set_refresh_interval(Some(Duration::from_millis(20)));
        profiler.record_draw_phases(DrawPhases::default());
        profiler.record_present_outcome(
            outcome,
            base + Duration::from_millis(21),
            base + Duration::from_millis(22),
            true,
            true,
        );
        profiler.record_frame_signal(None);
        let pending = profiler.pending_frame.unwrap();
        assert_eq!(pending.timing.signal_at, accepted.signal_at);
        assert_eq!(pending.timing.refresh_interval, accepted.refresh_interval);
        assert_eq!(pending.timing.phases, accepted.phases);
        assert_eq!(pending.timing.draw_start, accepted.draw_start);
        assert_eq!(pending.input_count, 1);
        assert!(profiler.input_latency_histogram.is_empty());
        let snapshot = profiler.frame_duration_snapshot();
        assert_eq!(snapshot.by_refresh_interval.len(), 1);
        assert!(snapshot.by_refresh_interval[0].work.is_empty());
    }
    let before_submit = collector.collect_unseen().entries;
    assert_eq!(
        before_submit
            .iter()
            .filter(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Present(_))
            ))
            .count(),
        3
    );
    assert!(!before_submit.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
    )));

    profiler.record_present_outcome(
        PlatformWindowPresentOutcome::Submitted,
        base + Duration::from_millis(25),
        base + Duration::from_millis(100),
        true,
        false,
    );
    let stats = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);
    assert_eq!(stats.refresh_interval, refresh);
    assert_eq!(stats.frames_drawn, 1);
    assert_eq!(stats.frames_late, 1);
    assert_eq!(stats.input_frames_late, 1);
    assert_eq!(stats.missed_refreshes, 2);
    assert_eq!(stats.work.count_at(25_000_000), 1);
    assert_eq!(stats.input_work.count_at(25_000_000), 1);
    assert_eq!(profiler.input_latency_histogram.count_at(100_000_000), 1);
    let input = profiler.input_latency_snapshot();
    assert_eq!(input.present_attempt_count, 4);
    assert_eq!(input.submitted_present_count, 1);
    assert_eq!(input.deferred_present_count, 1);
    assert_eq!(input.repaint_required_present_count, 1);
    assert_eq!(input.rejected_present_count, 1);
    let presented = collector
        .collect_unseen()
        .entries
        .into_iter()
        .find_map(|entry| match entry {
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented)) => {
                Some(presented)
            }
            _ => None,
        })
        .expect("submitted boundary");
    assert_eq!(presented.frame.signal_at, Some(base));
    assert_eq!(presented.frame.refresh_interval, refresh);
    assert_eq!(presented.frame.phases, phases);
}

#[test]
fn signal_and_refresh_belong_to_the_draw_that_consumed_them() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    let first_refresh = Some(Duration::from_millis(10));
    profiler.record_frame_signal(Some(base));
    profiler.set_refresh_interval(first_refresh);
    profiler.begin_draw();
    profiler.record_frame_signal(Some(base + Duration::from_millis(10)));
    profiler.set_refresh_interval(Some(Duration::from_millis(20)));
    profiler.end_draw(Some(base), 1);
    let accepted = profiler.pending_frame.unwrap().timing;
    assert_eq!(accepted.signal_at, Some(base));
    assert_eq!(accepted.refresh_interval, first_refresh);
    profiler.begin_draw();
    profiler.end_draw(Some(base), 1);
    let next = profiler.pending_frame.unwrap().timing;
    assert_eq!(next.signal_at, Some(base + Duration::from_millis(10)));
    assert_eq!(next.refresh_interval, Some(Duration::from_millis(20)));
    assert_eq!(
        profiler.frame_duration_snapshot().by_refresh_interval.len(),
        2
    );

    profiler.record_frame_signal(Some(base));
    profiler.record_frame_signal(None);
    profiler.set_refresh_interval(Some(Duration::ZERO));
    profiler.begin_draw();
    profiler.end_draw(None, 0);
    let next = profiler.pending_frame.unwrap().timing;
    assert_eq!(next.signal_at, None);
    assert_eq!(next.refresh_interval, None);
    assert_eq!(next.phases, DrawPhases::default());
}

#[test]
fn aborted_draws_clear_unfinished_views_and_phases_without_completing_old_frames() {
    for abort_mode in 0..3 {
        let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut profiler = profiler();
        let base = Instant::now();
        let accepted = frame_at(base, Some(Duration::from_millis(10)), Some(0));
        profiler.record_draw_timing(accepted);
        let checkpoint = profiler.checkpoint_draw();
        profiler.record_frame_signal(Some(base));
        profiler.begin_draw();
        profiler.begin_view();
        profiler.record_draw_phases(DrawPhases {
            paint: Duration::from_secs(1),
            ..DrawPhases::default()
        });
        match abort_mode {
            0 => profiler.abort_draw(),
            1 => profiler.abort_draw_checkpoint(checkpoint),
            _ => assert_eq!(
                profiler.finish_draw_checkpoint(checkpoint, None, 0, false),
                Duration::ZERO
            ),
        }
        assert!(profiler.active_views.is_empty());
        assert!(profiler.active_activities.is_empty());
        assert_eq!(
            profiler.pending_frame.unwrap().timing.signal_at,
            accepted.signal_at
        );
        assert_eq!(profiler.drawn_frame_count, 1);
        profiler.begin_draw();
        profiler.end_draw(None, 0);
        let next = profiler.pending_frame.unwrap().timing;
        assert_eq!(next.phases, DrawPhases::default());
        assert_eq!(next.signal_at, None);
    }
}

#[test]
fn only_inputs_covered_by_the_submitted_frame_classify_it_as_an_input_response() {
    for has_earlier_input in [false, true] {
        let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut profiler = profiler();
        let base = Instant::now();
        let refresh = Some(Duration::from_millis(10));
        if has_earlier_input {
            input_at(&mut profiler, base);
            profiler.end_input(true);
        }
        profiler.record_draw_timing(frame_at(base, refresh, Some(0)));
        input_at(&mut profiler, base + Duration::from_millis(2));
        profiler.end_input(true);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(5),
            base + Duration::from_millis(6),
            true,
            false,
        );
        let stats = profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .remove(0);
        assert_eq!(stats.input_frames_on_time, u64::from(has_earlier_input));
        assert_eq!(stats.input_work.len(), u64::from(has_earlier_input));
        assert_eq!(profiler.pending_input_count, 1);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(7),
            base + Duration::from_millis(8),
            true,
            false,
        );
        assert_eq!(
            profiler.frame_duration_snapshot().by_refresh_interval[0].frames_on_time,
            1
        );
        assert_eq!(profiler.pending_input_count, 1);
        profiler.record_draw_timing(frame_at(base + Duration::from_millis(10), refresh, Some(0)));
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(25),
            base + Duration::from_millis(26),
            true,
            false,
        );
        let stats = profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .remove(0);
        assert_eq!(stats.input_frames_late, 1);
        assert_eq!(stats.input_work.len(), u64::from(has_earlier_input) + 1);
        assert_eq!(profiler.pending_input_count, 0);
    }
}

#[test]
fn a_replacement_refresh_group_completes_all_pending_inputs_once() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    let sixty_hz = Some(Duration::from_secs(1) / 60);
    let two_forty_hz = Some(Duration::from_secs(1) / 240);
    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.record_draw_timing(frame_at(base, sixty_hz, Some(0)));
    profiler.record_present_outcome(
        PlatformWindowPresentOutcome::Deferred,
        base + Duration::from_millis(5),
        base + Duration::from_millis(6),
        true,
        false,
    );
    input_at(&mut profiler, base + Duration::from_millis(7));
    profiler.end_input(true);
    profiler.record_draw_timing(frame_at(
        base + Duration::from_millis(8),
        two_forty_hz,
        Some(0),
    ));
    profiler.record_present_outcome(
        PlatformWindowPresentOutcome::Submitted,
        base + Duration::from_millis(28),
        base + Duration::from_millis(29),
        true,
        false,
    );
    let frames = profiler.frame_duration_snapshot();
    assert_eq!(frames.by_refresh_interval.len(), 2);
    let old = &frames.by_refresh_interval[0];
    assert_eq!(old.refresh_interval, sixty_hz);
    assert_eq!(old.frames_drawn, 1);
    assert_eq!(
        old.frames_on_time + old.frames_late + old.frames_unmeasured,
        0
    );
    assert_eq!(
        old.work.len() + old.input_work.len() + old.missed_refreshes,
        0
    );
    let new = &frames.by_refresh_interval[1];
    assert_eq!(new.refresh_interval, two_forty_hz);
    assert_eq!(new.frames_drawn, 1);
    assert_eq!(new.frames_late, 1);
    assert_eq!(new.input_frames_late, 1);
    assert_eq!(new.frames_on_time + new.frames_unmeasured, 0);
    assert_eq!(new.missed_refreshes, 4);
    assert_eq!(new.work.count_at(20_000_000), 1);
    assert_eq!(new.input_work.count_at(20_000_000), 1);
    let inputs = profiler.input_latency_snapshot();
    assert_eq!(inputs.pending_input_count, 0);
    assert_eq!(inputs.latency_histogram.len(), 1);
    assert_eq!(inputs.events_per_frame_histogram.count_at(2), 1);
    assert_eq!(inputs.present_attempt_count, 2);
    assert_eq!(inputs.submitted_present_count, 1);
    assert_eq!(inputs.deferred_present_count, 1);
}

#[test]
fn nested_input_lifecycle_keeps_activity_until_the_outer_dispatch_finishes() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let mut collector = journal.collector();
    profiler.begin_input("key_down");
    profiler.begin_input("mouse_down");
    profiler.end_input(false);
    profiler.begin_input("mouse_move");
    profiler.end_input(false);
    let entries = collector.collect_unseen().entries;
    let states = entries
        .iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::InputActivity(activity) => Some(activity.active),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states, [true]);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Input(_))
            ))
            .count(),
        2
    );
    profiler.end_input(false);
    let entries = collector.collect_unseen().entries;
    let states = entries
        .iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::InputActivity(activity) => Some(activity.active),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states, [false]);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Input(_))
            ))
            .count(),
        1
    );
}

#[test]
fn input_activity_closes_on_draw_rollback_and_profiler_drop() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    {
        let mut profiler = profiler();
        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        profiler.begin_input("key_down");
        profiler.abort_draw_checkpoint(checkpoint);
        profiler.begin_input("mouse_down");
    }
    let entries = collector.collect_unseen().entries;
    let states = entries
        .iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::InputActivity(activity) => Some(activity.active),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states, [true, false, true, false]);
    let last_activity = entries
        .iter()
        .rposition(|entry| matches!(entry, ForegroundJournalEntry::InputActivity(_)))
        .unwrap();
    let closed = entries
        .iter()
        .position(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::FrameState(journal::FrameStateChange::Closed { .. })
            )
        })
        .unwrap();
    assert!(last_activity < closed);
}

#[test]
fn accepted_draw_inside_input_retains_exact_dispatch_coverage_for_refresh_statistics() {
    for after in [1, 2] {
        let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut profiler = profiler();
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.record_draw_timing(frame_at(base, Some(Duration::from_millis(10)), Some(0)));
        profiler.note_accepted_draw_covering_input(1);
        profiler.end_input_dispatch(0, after, true);
        profiler.record_present(
            base + Duration::from_millis(5),
            base + Duration::from_millis(6),
            true,
            false,
        );
        let stats = profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .remove(0);
        assert_eq!(stats.input_frames_on_time, u64::from(after == 1));
        assert_eq!(stats.input_work.len(), u64::from(after == 1));
        assert_eq!(profiler.pending_input_count, u64::from(after != 1));
    }
}

#[test]
fn reset_during_candidate_rollback_keeps_frame_statistics_and_never_revives_input() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    let refresh = Some(Duration::from_millis(10));
    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.record_draw_timing(frame_at(base, refresh, Some(0)));
    let checkpoint = profiler.checkpoint_draw();
    profiler.begin_draw();
    profiler.begin_view();
    profiler.reset_input_latency_measurements();
    profiler.reset_input_latency_measurements();
    profiler.abort_draw_checkpoint(checkpoint);
    assert!(profiler.active_views.is_empty());
    assert_eq!(profiler.pending_frame.unwrap().timing.signal_at, Some(base));
    profiler.record_present(
        base + Duration::from_millis(25),
        base + Duration::from_millis(26),
        true,
        false,
    );
    let stats = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);
    assert_eq!(stats.frames_drawn, 1);
    assert_eq!(stats.frames_late, 1);
    assert_eq!(stats.missed_refreshes, 2);
    assert!(stats.input_work.is_empty());
    assert_eq!(stats.input_frames_late, 0);
    assert!(profiler.input_latency_histogram.is_empty());
    assert_eq!(profiler.input_latency_snapshot().drawn_frame_count, 0);
}

#[test]
fn nested_views_record_own_time_without_double_counting_descendants() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    let at = |ms| base + Duration::from_millis(ms);
    begin_view_at(&mut profiler, at(0));
    begin_view_at(&mut profiler, at(2));
    begin_view_at(&mut profiler, at(3));
    profiler.end_view_at(at(3), "Grandchild", ViewPhase::Paint, at(6));
    profiler.end_view_at(at(2), "Child", ViewPhase::Prepaint, at(8));
    profiler.end_view_at(at(0), "Parent", ViewPhase::RequestLayout, at(10));
    assert!(profiler.active_views.is_empty());
    let timings = views(&collector.collect_unseen().entries);
    assert_eq!(timings.len(), 3);
    for (timing, name, phase, own_ms) in [
        (timings[0], "Grandchild", ViewPhase::Paint, 3),
        (timings[1], "Child", ViewPhase::Prepaint, 3),
        (timings[2], "Parent", ViewPhase::RequestLayout, 4),
    ] {
        assert_eq!(timing.name, name);
        assert_eq!(timing.phase, phase);
        assert_eq!(timing.self_duration, Duration::from_millis(own_ms));
    }
    assert_eq!(ViewPhase::RequestLayout.as_str(), "request_layout");
    assert_eq!(ViewPhase::Prepaint.as_str(), "prepaint");
    assert_eq!(ViewPhase::Paint.as_str(), "paint");
}

#[test]
fn view_floor_uses_own_time_and_unrecorded_children_still_reduce_the_parent() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    begin_view_at(&mut profiler, base);
    profiler.end_view_at(
        base,
        "BelowFloor",
        ViewPhase::Paint,
        base + VIEW_TIMING_FLOOR - Duration::from_nanos(1),
    );
    begin_view_at(&mut profiler, base);
    profiler.end_view_at(base, "AtFloor", ViewPhase::Paint, base + VIEW_TIMING_FLOOR);
    begin_view_at(&mut profiler, base);
    begin_view_at(&mut profiler, base + Duration::from_micros(100));
    profiler.end_view_at(
        base + Duration::from_micros(100),
        "SmallChild",
        ViewPhase::Paint,
        base + Duration::from_micros(500),
    );
    profiler.end_view_at(
        base,
        "SmallParent",
        ViewPhase::Paint,
        base + Duration::from_micros(750),
    );
    let timings = views(&collector.collect_unseen().entries);
    assert_eq!(timings.len(), 1);
    assert_eq!(timings[0].name, "AtFloor");
    assert_eq!(timings[0].self_duration, VIEW_TIMING_FLOOR);
}

#[test]
fn nested_view_unwind_retains_actual_work_without_accepting_a_frame() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    let at = |ms| base + Duration::from_millis(ms);
    let checkpoint = profiler.checkpoint_draw();
    profiler.begin_draw();
    begin_view_at(&mut profiler, at(0));
    let outer = catch_unwind(AssertUnwindSafe(|| {
        begin_view_at(&mut profiler, at(2));
        let inner = catch_unwind(|| panic!("view body unwound"));
        profiler.end_view_at(at(2), "UnwoundChild", ViewPhase::Prepaint, at(8));
        resume_unwind(inner.unwrap_err());
    }));
    profiler.end_view_at(at(0), "UnwoundParent", ViewPhase::RequestLayout, at(10));
    assert!(outer.is_err());
    profiler.abort_draw_checkpoint(checkpoint);
    assert!(profiler.active_views.is_empty());
    assert!(profiler.active_activities.is_empty());
    assert!(profiler.pending_frame.is_none());
    assert!(
        profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .is_empty()
    );
    let entries = collector.collect_unseen().entries;
    let timings = views(&entries);
    assert_eq!(timings.len(), 2);
    assert_eq!(timings[0].self_duration, Duration::from_millis(6));
    assert_eq!(timings[1].self_duration, Duration::from_millis(4));
    assert!(!entries.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Event(ForegroundEvent::Draw(_))
            | ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
    )));
}

#[test]
fn nested_draw_rollback_preserves_outer_metadata_and_completed_nested_view_work() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    let at = |ms| base + Duration::from_millis(ms);
    let refresh = Some(Duration::from_millis(10));
    let phases = DrawPhases {
        paint: Duration::from_millis(3),
        ..DrawPhases::default()
    };
    profiler.record_frame_signal(Some(base));
    profiler.set_refresh_interval(refresh);
    profiler.begin_draw();
    profiler.record_draw_phases(phases);
    begin_view_at(&mut profiler, at(0));
    let checkpoint = profiler.checkpoint_draw();
    profiler.record_frame_signal(Some(at(1)));
    profiler.set_refresh_interval(Some(Duration::from_millis(20)));
    profiler.begin_draw();
    profiler.record_draw_phases(DrawPhases::default());
    begin_view_at(&mut profiler, at(2));
    profiler.end_view_at(at(2), "NestedDrawView", ViewPhase::Paint, at(8));
    profiler.abort_draw_checkpoint(checkpoint);
    assert_eq!(profiler.active_views.len(), 1);
    profiler.end_view_at(at(0), "OuterView", ViewPhase::Paint, at(10));
    profiler.end_draw(Some(base), 1);
    let accepted = profiler.pending_frame.unwrap().timing;
    assert_eq!(accepted.refresh_interval, refresh);
    assert_eq!(accepted.signal_at, Some(base));
    assert_eq!(accepted.phases, phases);
    assert_eq!(profiler.drawn_frame_count, 1);
    let timings = views(&collector.collect_unseen().entries);
    assert_eq!(timings.len(), 2);
    assert_eq!(timings[1].name, "OuterView");
    assert_eq!(timings[1].self_duration, Duration::from_millis(4));
}

#[test]
fn reset_preserves_live_view_scopes_and_does_not_change_their_own_time() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let base = Instant::now();
    profiler.begin_draw();
    begin_view_at(&mut profiler, base);
    begin_view_at(&mut profiler, base + Duration::from_millis(2));
    profiler.reset_input_latency_measurements();
    profiler.end_view_at(
        base + Duration::from_millis(2),
        "Child",
        ViewPhase::Paint,
        base + Duration::from_millis(8),
    );
    profiler.end_view_at(
        base,
        "Parent",
        ViewPhase::Paint,
        base + Duration::from_millis(10),
    );
    profiler.end_draw(Some(base), 1);
    let timings = views(&collector.collect_unseen().entries);
    assert_eq!(timings.len(), 2);
    assert_eq!(timings[0].self_duration, Duration::from_millis(6));
    assert_eq!(timings[1].self_duration, Duration::from_millis(4));
    assert!(profiler.active_views.is_empty());
    assert_eq!(
        profiler.frame_duration_snapshot().by_refresh_interval[0].frames_drawn,
        1
    );
}

#[test]
fn interrupted_draws_discard_view_work_and_cannot_reuse_their_signal_or_phases() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let old = Instant::now() - Duration::from_secs(1);
    profiler.record_frame_signal(Some(old));
    profiler.begin_draw();
    begin_view_at(&mut profiler, old);
    profiler.record_draw_phases(DrawPhases {
        paint: Duration::from_secs(1),
        ..DrawPhases::default()
    });
    journal::record_power_transition(journal::PowerState::Suspended);
    journal::record_power_transition(journal::PowerState::Awake);
    profiler.end_view_at(old, "SuspendedView", ViewPhase::Paint, Instant::now());
    profiler.end_draw(Some(old), 1);
    assert!(views(&collector.collect_unseen().entries).is_empty());
    assert!(profiler.active_views.is_empty());
    assert!(profiler.pending_frame.is_none());
    assert_eq!(profiler.drawn_frame_count, 0);
    profiler.begin_draw();
    profiler.end_draw(Some(Instant::now()), 1);
    let accepted = profiler.pending_frame.unwrap().timing;
    assert_eq!(accepted.signal_at, None);
    assert_eq!(accepted.phases, DrawPhases::default());
}

#[test]
fn late_signals_and_submission_wait_do_not_inflate_frame_work() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.record_draw_timing(FrameTiming {
        draw_start: base + Duration::from_millis(45),
        draw_end: base + Duration::from_millis(46),
        ..frame_at(base, Some(Duration::from_millis(10)), Some(40))
    });
    profiler.record_present(
        base + Duration::from_millis(48),
        base + Duration::from_millis(200),
        true,
        false,
    );
    let snapshot = profiler.frame_duration_snapshot();
    let stats = &snapshot.by_refresh_interval[0];
    assert_eq!(stats.frames_on_time, 1);
    assert_eq!(stats.input_frames_on_time, 1);
    assert_eq!(stats.frames_late, 0);
    assert_eq!(stats.missed_refreshes, 0);
    assert_eq!(stats.work.count_at(8_000_000), 1);
    assert_eq!(stats.input_work.count_at(8_000_000), 1);
    assert_eq!(snapshot.draw_duration_histogram.count_at(1_000_000), 1);
    assert_eq!(snapshot.dirty_to_present_histogram.count_at(200_000_000), 1);
    assert_eq!(profiler.input_latency_histogram.count_at(200_000_000), 1);
}

#[test]
fn input_latency_remains_measured_when_frame_work_has_no_signal() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    input_at(&mut profiler, base);
    profiler.end_input(true);
    profiler.record_draw_timing(frame_at(base, None, None));
    profiler.record_present(
        base + Duration::from_millis(25),
        base + Duration::from_millis(30),
        true,
        false,
    );
    let stats = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);
    assert_eq!(stats.refresh_interval, None);
    assert_eq!(stats.frames_unmeasured, 1);
    assert_eq!(stats.frames_on_time, 0);
    assert_eq!(stats.frames_late, 0);
    assert_eq!(stats.input_frames_on_time, 0);
    assert_eq!(stats.input_frames_late, 0);
    assert!(stats.work.is_empty());
    assert!(stats.input_work.is_empty());
    assert_eq!(profiler.input_latency_histogram.count_at(30_000_000), 1);
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
}

#[test]
fn every_accepted_draw_is_recorded_even_without_a_submission() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    let base = Instant::now();
    for draw_ms in [2, 40] {
        profiler.record_draw_timing(FrameTiming {
            draw_end: base + Duration::from_millis(draw_ms),
            ..frame_at(base, Some(Duration::from_millis(10)), None)
        });
    }
    let snapshot = profiler.frame_duration_snapshot();
    assert_eq!(snapshot.draw_duration_histogram.len(), 2);
    assert_eq!(snapshot.draw_duration_histogram.count_at(2_000_000), 1);
    assert_eq!(snapshot.draw_duration_histogram.count_at(40_000_000), 1);
    let stats = &snapshot.by_refresh_interval[0];
    assert_eq!(stats.frames_drawn, 2);
    assert_eq!(stats.draw_duration.len(), 2);
    assert_eq!(
        stats.frames_on_time + stats.frames_late + stats.frames_unmeasured,
        0
    );
    assert!(stats.work.is_empty());
}

#[test]
fn signals_from_old_visibility_or_power_epochs_cannot_measure_new_frame_work() {
    for power_interruption in [false, true] {
        let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut profiler = profiler();
        let old_signal = Instant::now() - Duration::from_secs(1);
        if power_interruption {
            journal::record_power_transition(journal::PowerState::Suspended);
            journal::record_power_transition(journal::PowerState::Awake);
        } else {
            journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Hidden);
            journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Visible);
        }
        let base = Instant::now() + Duration::from_millis(1);
        profiler.record_draw_timing(FrameTiming {
            signal_at: Some(old_signal),
            ..frame_at(base, Some(Duration::from_millis(10)), None)
        });
        let accepted = profiler.pending_frame.unwrap().timing;
        assert_eq!(accepted.signal_at, None);
        assert_eq!(accepted.work(base + Duration::from_millis(25)), None);
        profiler.record_present(
            base + Duration::from_millis(25),
            base + Duration::from_millis(26),
            true,
            false,
        );
        let stats = profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .remove(0);
        assert_eq!(stats.frames_drawn, 1);
        assert_eq!(stats.frames_unmeasured, 1);
        assert_eq!(stats.frames_on_time + stats.frames_late, 0);
        assert!(stats.work.is_empty());
        assert_eq!(profiler.draw_duration_histogram.len(), 1);
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
    }
}

#[test]
fn hidden_draws_keep_actual_draw_phases_without_measuring_signal_latency() {
    let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut profiler = profiler();
    journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Hidden);
    let phases = DrawPhases {
        paint: Duration::from_millis(1),
        ..DrawPhases::default()
    };
    let signal = Instant::now();
    profiler.record_frame_signal(Some(signal));
    profiler.begin_draw();
    profiler.record_draw_phases(phases);
    profiler.end_draw(Some(signal), 1);
    let hidden = profiler.pending_frame.unwrap().timing;
    assert_eq!(hidden.signal_at, None);
    assert_eq!(hidden.dirty_at, None);
    assert_eq!(hidden.phases, phases);
    let now = Instant::now();
    profiler.record_present(now, now, true, false);
    let stats = profiler
        .frame_duration_snapshot()
        .by_refresh_interval
        .remove(0);
    assert_eq!(stats.frames_drawn, 1);
    assert_eq!(
        stats.frames_on_time + stats.frames_late + stats.frames_unmeasured,
        0
    );
    assert_eq!(profiler.draw_duration_histogram.len(), 1);
    assert!(profiler.dirty_to_present_histogram.is_empty());
    assert!(profiler.input_latency_histogram.is_empty());
}

#[test]
fn missed_refresh_totals_saturate_without_losing_the_frame_sample() {
    let base = Instant::now();
    let mut stats =
        RefreshIntervalFrames::new(Some(Duration::from_millis(10)), &Histogram::new(3).unwrap());
    stats.missed_refreshes = u64::MAX - 1;
    stats.record_presented_frame(
        &frame_at(base, Some(Duration::from_millis(10)), Some(0)),
        &PresentTiming {
            window_id: WindowId::from(0xF3A0),
            present_start: base + Duration::from_millis(25),
            present_end: base + Duration::from_millis(26),
            animation_interval: None,
        },
        false,
    );
    assert_eq!(stats.missed_refreshes, u64::MAX);
    assert_eq!(stats.frames_late, 1);
    assert_eq!(stats.work.len(), 1);
}
