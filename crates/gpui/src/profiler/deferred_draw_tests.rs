use super::*;
use journal::{ForegroundEvent, ForegroundJournalEntry, IntervalBoundary};

fn profiler() -> WindowProfiler {
    WindowProfiler::new(WindowId::from(0xF3D0)).expect("valid histograms")
}

fn invalidate(profiler: &mut WindowProfiler) -> Instant {
    profiler.begin_input("key_down");
    let Some(WindowActivity::Input { started_at, .. }) = profiler.active_activities.last() else {
        panic!("input must be active");
    };
    let started_at = *started_at;
    profiler.end_input(true);
    started_at
}

fn prepare(profiler: &mut WindowProfiler, update_count_at_draw_start: usize) -> PreparedDrawTiming {
    let checkpoint = profiler.checkpoint_draw();
    profiler.begin_draw();
    profiler
        .prepare_draw_checkpoint(checkpoint, None, 1, update_count_at_draw_start)
        .expect("completed candidate draw")
}

fn submit(profiler: &mut WindowProfiler, at: Instant) {
    profiler.record_present_outcome(PlatformWindowPresentOutcome::Submitted, at, at, true, false);
}

#[test]
fn prepared_draw_publishes_original_timing_only_after_acceptance() {
    let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut collector = journal.collector();
    let mut profiler = profiler();
    let first_input_at = invalidate(&mut profiler);
    invalidate(&mut profiler);
    let signal_at = Instant::now();
    let refresh_interval = Some(Duration::from_millis(10));
    let phases = DrawPhases {
        request_layout: Duration::from_micros(1),
        prepaint: Duration::from_micros(2),
        paint: Duration::from_micros(3),
    };
    profiler.set_refresh_interval(refresh_interval);
    profiler.record_frame_signal(Some(signal_at));
    let checkpoint = profiler.checkpoint_draw();
    profiler.begin_draw();
    profiler.record_draw_phases(phases);
    let before_prepare = Instant::now();
    let prepared = profiler
        .prepare_draw_checkpoint(checkpoint, Some(first_input_at), 7, 19)
        .expect("completed candidate draw");
    let original = prepared.frame.timing;
    assert!(original.draw_end >= before_prepare);
    assert!(original.draw_end <= Instant::now());
    assert!(profiler.active_activities.is_empty());
    assert!(profiler.active_views.is_empty());
    assert!(profiler.pending_frame.is_none());
    assert_eq!(profiler.pending_input_count, 2);
    assert_eq!(profiler.drawn_frame_count, 0);
    assert!(profiler.draw_duration_histogram.is_empty());
    assert!(profiler.input_latency_histogram.is_empty());
    assert!(profiler.events_per_frame_histogram.is_empty());
    assert!(
        profiler
            .frame_duration_snapshot()
            .by_refresh_interval
            .is_empty()
    );
    assert!(
        !collector
            .collect_unseen()
            .entries
            .iter()
            .any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(_))
            ))
    );

    journal::clear_frame_pending(profiler.window_id);
    {
        let _turn = journal::foreground_turn();
        let now = Instant::now();
        journal::record_input(journal::InputTiming {
            kind: "idle_probe",
            start: now,
            end: now,
            caused_invalidation: false,
        });
    }
    assert!(
        collector
            .collect_unseen()
            .entries
            .iter()
            .any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
            ))
    );

    profiler.set_refresh_interval(Some(Duration::from_millis(20)));
    profiler.record_frame_signal(Some(Instant::now()));
    assert_eq!(
        profiler.accept_prepared_draw(prepared, 19),
        original.draw_duration()
    );
    let accepted = profiler.pending_frame.expect("accepted timing");
    assert_eq!(accepted.timing.draw_start, original.draw_start);
    assert_eq!(accepted.timing.draw_end, original.draw_end);
    assert_eq!(accepted.timing.dirty_at, Some(first_input_at));
    assert_eq!(accepted.timing.invalidations, 7);
    assert_eq!(accepted.timing.signal_at, Some(signal_at));
    assert_eq!(accepted.timing.refresh_interval, refresh_interval);
    assert_eq!(accepted.timing.phases, phases);
    assert_eq!(accepted.first_input_at, Some(first_input_at));
    assert_eq!(accepted.input_count, 2);
    assert_eq!(profiler.drawn_frame_count, 1);
    assert_eq!(profiler.draw_duration_histogram.len(), 1);
    assert_eq!(
        profiler
            .draw_duration_histogram
            .count_at(original.draw_duration().as_nanos() as u64),
        1
    );
    assert!(profiler.input_latency_histogram.is_empty());
    let entries = collector.collect_unseen().entries;
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(timing))
                    if timing.draw_end == original.draw_end
                        && timing.draw_start == original.draw_start
            ))
            .count(),
        1
    );

    let settled_at = original.draw_end + Duration::from_secs(5);
    submit(&mut profiler, settled_at);
    assert_eq!(profiler.draw_duration_histogram.len(), 1);
    assert_eq!(profiler.input_latency_histogram.len(), 1);
    assert_eq!(
        profiler
            .input_latency_histogram
            .count_at(settled_at.duration_since(first_input_at).as_nanos() as u64),
        1
    );
    assert_eq!(profiler.events_per_frame_histogram.count_at(2), 1);
    assert_eq!(profiler.pending_input_count, 0);
}

#[test]
fn cancelling_prepared_draw_preserves_the_previous_frame_and_its_input_boundary() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    let first_input_at = invalidate(&mut profiler);
    profiler.begin_draw();
    profiler.end_draw(Some(first_input_at), 1);
    let previous = profiler.pending_frame.expect("previous accepted frame");
    let second_input_at = invalidate(&mut profiler);
    let prepared = prepare(&mut profiler, 19);
    invalidate(&mut profiler);
    drop(prepared);
    invalidate(&mut profiler);

    let pending = profiler
        .pending_frame
        .expect("previous accepted frame retained");
    assert_eq!(pending.timing.draw_start, previous.timing.draw_start);
    assert_eq!(pending.timing.draw_end, previous.timing.draw_end);
    assert_eq!(pending.first_input_at, previous.first_input_at);
    assert_eq!(pending.input_count, previous.input_count);
    assert_eq!(profiler.input_after_draw_at, Some(second_input_at));
    assert_eq!(profiler.pending_input_count, 4);
    assert_eq!(profiler.drawn_frame_count, 1);
    assert_eq!(profiler.draw_duration_histogram.len(), 1);
    assert!(profiler.input_latency_histogram.is_empty());
    assert!(profiler.events_per_frame_histogram.is_empty());
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 3);
    assert_eq!(profiler.first_input_at, Some(second_input_at));
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
    profiler.begin_draw();
    profiler.end_draw(None, 1);
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 0);
    assert_eq!(profiler.events_per_frame_histogram.count_at(3), 1);
}

#[test]
fn inputs_received_while_prepared_remain_pending_for_a_later_draw() {
    for covered_inputs in [0, 2] {
        let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut profiler = profiler();
        for _ in 0..covered_inputs {
            invalidate(&mut profiler);
        }
        let prepared = prepare(&mut profiler, 19);
        let first_later_input_at = invalidate(&mut profiler);
        invalidate(&mut profiler);
        profiler.accept_prepared_draw(prepared, 19);
        assert_eq!(
            profiler.pending_frame.expect("accepted timing").input_count,
            covered_inputs
        );
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 2);
        assert_eq!(profiler.first_input_at, Some(first_later_input_at));
        assert_eq!(
            profiler.input_latency_histogram.len(),
            u64::from(covered_inputs > 0)
        );
        assert_eq!(
            profiler.events_per_frame_histogram.len(),
            u64::from(covered_inputs > 0)
        );
        profiler.begin_draw();
        profiler.end_draw(None, 1);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.first_input_at, None);
        assert_eq!(
            profiler.events_per_frame_histogram.count_at(2),
            1 + u64::from(covered_inputs == 2)
        );
    }
}

#[test]
fn input_reset_during_retention_cannot_restore_original_input_coverage() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    invalidate(&mut profiler);
    let prepared = prepare(&mut profiler, 19);
    let original_draw_end = prepared.frame.timing.draw_end;
    profiler.reset_input_latency_measurements();
    profiler.reset_input_latency_measurements();
    let new_input_at = invalidate(&mut profiler);
    profiler.accept_prepared_draw(prepared, 19);
    let accepted = profiler
        .pending_frame
        .expect("frame timing survives input reset");
    assert_eq!(accepted.timing.draw_end, original_draw_end);
    assert_eq!(accepted.first_input_at, None);
    assert_eq!(accepted.input_count, 0);
    assert_eq!(profiler.drawn_frame_count, 1);
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 1);
    assert_eq!(profiler.first_input_at, Some(new_input_at));
    assert!(profiler.input_latency_histogram.is_empty());
    assert!(profiler.events_per_frame_histogram.is_empty());
    profiler.begin_draw();
    profiler.end_draw(None, 1);
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 0);
    assert_eq!(profiler.input_latency_histogram.len(), 1);
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
}

#[test]
fn power_interruption_during_retention_cannot_publish_old_draw_or_input_samples() {
    for notified in [false, true] {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler();
        invalidate(&mut profiler);
        let prepared = prepare(&mut profiler, 19);
        if notified {
            journal::record_power_transition(journal::PowerState::Suspended);
            journal::record_power_transition(journal::PowerState::Awake);
        } else {
            journal::advance_test_suspended_time(Duration::from_secs(2));
        }
        profiler.accept_prepared_draw(prepared, 19);
        assert!(profiler.pending_frame.is_none());
        assert_eq!(profiler.drawn_frame_count, 0);
        assert!(profiler.draw_duration_histogram.is_empty());
        assert_eq!(profiler.pending_input_count, 0);
        let new_input_at = invalidate(&mut profiler);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 1);
        assert_eq!(profiler.first_input_at, Some(new_input_at));
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.events_per_frame_histogram.is_empty());
        assert!(
            !collector
                .collect_unseen()
                .entries
                .iter()
                .any(|entry| matches!(
                    entry,
                    ForegroundJournalEntry::Event(ForegroundEvent::Draw(_))
                        | ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
                ))
        );
        profiler.begin_draw();
        profiler.end_draw(None, 1);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
    }
}

#[test]
fn acceptance_only_marks_preexisting_active_inputs_with_exact_update_coverage() {
    for (input_precedes_draw, after) in [(true, 1), (true, 2), (false, 1)] {
        let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut profiler = profiler();
        if input_precedes_draw {
            profiler.begin_input("original_input");
        }
        let prepared = prepare(&mut profiler, 1);
        if !input_precedes_draw {
            profiler.begin_input("later_input");
        }
        profiler.accept_prepared_draw(prepared, 1);
        profiler.end_input_dispatch(0, after, true);
        let covered = input_precedes_draw && after == 1;
        assert_eq!(
            profiler.pending_frame.expect("accepted timing").input_count,
            u64::from(covered)
        );
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, u64::from(!covered));
        assert_eq!(profiler.input_latency_histogram.len(), u64::from(covered));
    }
}

#[test]
fn original_input_ending_before_acceptance_is_covered_without_consuming_later_input() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    profiler.begin_input("original_input");
    let Some(WindowActivity::Input { started_at, .. }) = profiler.active_activities.last() else {
        panic!("input must be active");
    };
    let original_input_at = *started_at;
    let prepared = prepare(&mut profiler, 1);
    let original_draw = prepared.frame.timing;
    profiler.end_input_dispatch(0, 1, true);
    let later_input_at = invalidate(&mut profiler);
    assert!(profiler.active_activities.is_empty());
    assert!(profiler.pending_frame.is_none());
    assert_eq!(profiler.pending_input_count, 2);
    assert_eq!(profiler.drawn_frame_count, 0);
    assert!(profiler.draw_duration_histogram.is_empty());
    assert!(profiler.input_latency_histogram.is_empty());

    profiler.accept_prepared_draw(prepared, 1);
    let accepted = profiler.pending_frame.expect("accepted timing");
    assert_eq!(accepted.timing.draw_start, original_draw.draw_start);
    assert_eq!(accepted.timing.draw_end, original_draw.draw_end);
    assert_eq!(accepted.first_input_at, Some(original_input_at));
    assert_eq!(accepted.input_count, 1);
    let settled_at = Instant::now();
    submit(&mut profiler, settled_at);
    assert_eq!(profiler.pending_input_count, 1);
    assert_eq!(profiler.first_input_at, Some(later_input_at));
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
    assert_eq!(
        profiler
            .input_latency_histogram
            .count_at(settled_at.duration_since(original_input_at).as_nanos() as u64),
        1
    );
    profiler.begin_draw();
    profiler.end_draw(None, 1);
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 0);
    assert_eq!(profiler.first_input_at, None);
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 2);
}

#[test]
fn original_input_ending_before_acceptance_retains_invalidations_after_the_draw() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    profiler.begin_input("original_input");
    let prepared = prepare(&mut profiler, 1);
    profiler.end_input_dispatch(0, 2, true);
    profiler.accept_prepared_draw(prepared, 1);
    assert_eq!(
        profiler.pending_frame.expect("accepted timing").input_count,
        0
    );
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 1);
    assert!(profiler.input_latency_histogram.is_empty());
    assert!(profiler.events_per_frame_histogram.is_empty());
    profiler.begin_draw();
    profiler.end_draw(None, 1);
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 0);
    assert_eq!(profiler.input_latency_histogram.len(), 1);
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
}

#[test]
fn later_input_with_the_same_timestamp_cannot_claim_prepared_draw_coverage() {
    for finishes_before_acceptance in [false, true] {
        let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut profiler = profiler();
        let prepared = prepare(&mut profiler, 1);
        let draw_start = prepared.frame.timing.draw_start;
        profiler.begin_input("later_input");
        let Some(WindowActivity::Input { started_at, .. }) = profiler.active_activities.last_mut()
        else {
            panic!("input must be active");
        };
        *started_at = draw_start;
        if finishes_before_acceptance {
            profiler.end_input_dispatch(0, 1, true);
        }
        profiler.accept_prepared_draw(prepared, 1);
        if !finishes_before_acceptance {
            profiler.end_input_dispatch(0, 1, true);
        }
        assert_eq!(
            profiler.pending_frame.expect("accepted timing").input_count,
            0
        );
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 1);
        assert_eq!(profiler.first_input_at, Some(draw_start));
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.events_per_frame_histogram.is_empty());
        profiler.begin_draw();
        profiler.end_draw(None, 1);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
    }
}

#[test]
fn input_reset_invalidates_shared_coverage_whether_original_input_has_finished() {
    for finishes_before_reset in [false, true] {
        let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut profiler = profiler();
        profiler.begin_input("original_input");
        let prepared = prepare(&mut profiler, 1);
        if finishes_before_reset {
            profiler.end_input_dispatch(0, 1, true);
        }
        profiler.reset_input_latency_measurements();
        if !finishes_before_reset {
            profiler.end_input_dispatch(0, 1, true);
        }
        let new_input_at = invalidate(&mut profiler);
        profiler.accept_prepared_draw(prepared, 1);
        let accepted = profiler.pending_frame.expect("draw survives input reset");
        assert_eq!(accepted.first_input_at, None);
        assert_eq!(accepted.input_count, 0);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 1);
        assert_eq!(profiler.first_input_at, Some(new_input_at));
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.events_per_frame_histogram.is_empty());
        profiler.begin_draw();
        profiler.end_draw(None, 1);
        submit(&mut profiler, Instant::now());
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
    }
}

#[test]
fn power_interruption_invalidates_shared_coverage_whether_original_input_has_finished() {
    for notified in [false, true] {
        for finishes_before_interruption in [false, true] {
            let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
            let mut profiler = profiler();
            profiler.begin_input("original_input");
            let prepared = prepare(&mut profiler, 1);
            if finishes_before_interruption {
                profiler.end_input_dispatch(0, 1, true);
            }
            if notified {
                journal::record_power_transition(journal::PowerState::Suspended);
                journal::record_power_transition(journal::PowerState::Awake);
            } else {
                journal::advance_test_suspended_time(Duration::from_secs(2));
            }
            if !finishes_before_interruption {
                profiler.end_input_dispatch(0, 1, true);
            }
            let new_input_at = invalidate(&mut profiler);
            profiler.accept_prepared_draw(prepared, 1);
            assert!(profiler.pending_frame.is_none());
            assert_eq!(profiler.drawn_frame_count, 0);
            assert!(profiler.draw_duration_histogram.is_empty());
            submit(&mut profiler, Instant::now());
            assert_eq!(profiler.pending_input_count, 1);
            assert_eq!(profiler.first_input_at, Some(new_input_at));
            assert!(profiler.input_latency_histogram.is_empty());
            assert!(profiler.events_per_frame_histogram.is_empty());
            profiler.begin_draw();
            profiler.end_draw(None, 1);
            submit(&mut profiler, Instant::now());
            assert_eq!(profiler.pending_input_count, 0);
            assert_eq!(profiler.input_latency_histogram.len(), 1);
            assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
        }
    }
}

#[test]
fn cancelled_prepared_coverage_preserves_the_original_inputs_accepted_draw() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    profiler.begin_input("original_input");
    profiler.begin_draw();
    profiler.end_draw(None, 1);
    profiler.note_accepted_draw_covering_input(1);
    let previous = profiler.pending_frame.expect("previous accepted frame");
    let prepared = prepare(&mut profiler, 1);
    profiler.end_input_dispatch(0, 1, true);
    drop(prepared);
    let pending = profiler
        .pending_frame
        .expect("previous accepted frame retained");
    assert_eq!(pending.timing.draw_start, previous.timing.draw_start);
    assert_eq!(pending.timing.draw_end, previous.timing.draw_end);
    assert_eq!(pending.input_count, 1);
    assert_eq!(profiler.pending_input_count, 1);
    assert_eq!(profiler.drawn_frame_count, 1);
    assert_eq!(profiler.draw_duration_histogram.len(), 1);
    assert!(profiler.input_latency_histogram.is_empty());
    submit(&mut profiler, Instant::now());
    assert_eq!(profiler.pending_input_count, 0);
    assert_eq!(profiler.input_latency_histogram.len(), 1);
    assert_eq!(profiler.events_per_frame_histogram.count_at(1), 1);
}

#[test]
fn preparing_a_nested_draw_restores_the_enclosing_draw_and_view_scopes() {
    let (_journal, _guard) = journal::install_test_foreground_journal(128, 4);
    let mut profiler = profiler();
    let outer_phases = DrawPhases {
        paint: Duration::from_micros(3),
        ..DrawPhases::default()
    };
    profiler.begin_draw();
    profiler.record_draw_phases(outer_phases);
    let outer_view = profiler.begin_view();
    let prepared = prepare(&mut profiler, 19);
    assert_eq!(profiler.active_activities.len(), 1);
    assert_eq!(profiler.active_views.len(), 1);
    assert!(matches!(
        profiler.active_activities.last(),
        Some(WindowActivity::Draw { phases, .. }) if *phases == outer_phases
    ));
    assert_eq!(profiler.drawn_frame_count, 0);
    drop(prepared);
    profiler.end_view(outer_view, "OuterView", ViewPhase::Paint);
    profiler.end_draw(None, 1);
    assert!(profiler.active_activities.is_empty());
    assert!(profiler.active_views.is_empty());
    assert_eq!(profiler.drawn_frame_count, 1);
}
