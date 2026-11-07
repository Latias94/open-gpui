use super::*;

#[cfg(not(feature = "profiler"))]
#[test]
fn disabled_task_scopes_do_not_register_timings_even_when_they_unwind() {
    std::thread::spawn(|| {
        let thread_id = std::thread::current().id();
        let result = std::panic::catch_unwind(|| {
            let _timing = task_timing(SpawnTime(Instant::now()), std::panic::Location::caller());
            let _unqueued =
                task_timing_unqueued(SpawnTime(Instant::now()), std::panic::Location::caller());
            panic!("task scope unwind");
        });
        assert!(result.is_err());
        assert_eq!(std::mem::size_of::<TaskTimingGuard>(), 0);
        assert!(
            get_current_thread_timings(TasksIncluded::CompletedAndRunning)
                .timings
                .is_empty()
        );
        assert!(
            GLOBAL_THREAD_TIMINGS
                .lock()
                .iter()
                .all(|thread| thread.thread_id != thread_id)
        );
    })
    .join()
    .unwrap();
}

#[cfg(feature = "profiler")]
mod enabled {
    use super::*;
    use journal::{
        ForegroundEvent, ForegroundJournalCollector, ForegroundJournalEntry, FrameStateChange,
        IntervalBoundary,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn profiler(id: u64) -> WindowProfiler {
        WindowProfiler::new(WindowId::from(id)).unwrap()
    }

    fn draw_at(profiler: &mut WindowProfiler, start: Instant) {
        profiler.record_draw_timing(FrameTiming {
            window_id: profiler.window_id,
            dirty_at: Some(start),
            invalidations: 3,
            draw_start: start + Duration::from_millis(2),
            draw_end: start + Duration::from_millis(4),
            refresh_interval: None,
            signal_at: None,
            phases: DrawPhases::default(),
        });
    }

    fn input_at(profiler: &mut WindowProfiler, started_at: Instant, kind: &'static str) {
        profiler.begin_input(kind);
        let Some(WindowActivity::Input {
            started_at: active_start,
            ..
        }) = profiler.active_activities.last_mut()
        else {
            panic!("input must be active");
        };
        *active_start = started_at;
    }

    fn present(profiler: &mut WindowProfiler, outcome: PlatformWindowPresentOutcome) {
        let _turn = journal::foreground_turn();
        let now = Instant::now();
        profiler.record_present_outcome(outcome, now, now, true, true);
    }

    fn has_idle(entries: &[ForegroundJournalEntry]) -> bool {
        entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
            )
        })
    }

    fn assert_idle_probe(collector: &mut ForegroundJournalCollector) {
        let turn = journal::foreground_turn();
        let now = Instant::now();
        journal::record_input(journal::InputTiming {
            kind: "probe",
            start: now,
            end: now,
            caused_invalidation: false,
        });
        drop(turn);
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(has_idle(&drained.entries), "{drained:?}");
    }

    #[test]
    fn saved_task_timing_returns_the_actual_completed_poll() {
        std::thread::spawn(|| {
            let start = Instant::now();
            let spawned = SpawnTime(start - Duration::from_secs(1));
            let location = std::panic::Location::caller();
            let mut timings = ThreadTimings::new(None, std::thread::current().id());
            timings.running = Some(ActiveTiming {
                location,
                spawned,
                start,
            });
            let ended = YieldTime(start + Duration::from_millis(5));
            let timing = timings.save_task_timing(ended);
            assert_eq!(timing.location, location);
            assert_eq!(timing.spawned.0, spawned.0);
            assert_eq!(timing.start, start);
            assert_eq!(timing.end.0, ended.0);
            assert!(timings.running.is_none());
            assert!(
                timings
                    .stats
                    .longest_poll_times
                    .iter()
                    .any(|sample| sample.start == start && sample.end.0 == ended.0)
            );
        })
        .join()
        .unwrap();
    }

    #[test]
    fn task_scope_finishes_a_runnable_that_propagates_its_panic() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let metadata = crate::RunnableMeta {
            location: std::panic::Location::caller(),
            spawned: SpawnTime(Instant::now()),
        };
        let (runnable, task) = async_task::Builder::new()
            .metadata(metadata.clone())
            .spawn(|_| async { panic!("runnable poll unwind") }, |_| {});
        journal::foreground_runnable_counter().queued();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _timing = task_timing(runnable.metadata().spawned, runnable.metadata().location);
            THREAD_TIMINGS.with(|timings| {
                timings.lock().running.as_mut().unwrap().start =
                    Instant::now() - Duration::from_millis(1);
            });
            runnable.run();
        }));
        assert!(result.is_err());
        drop(task);
        assert!(THREAD_TIMINGS.with(|timings| timings.lock().running.is_none()));
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::TaskPoll(timing))
                if timing.location == metadata.location && timing.spawned.0 == metadata.spawned.0
        )));
        assert!(has_idle(&entries), "{entries:?}");
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn nested_task_scopes_restore_the_enclosing_poll_after_unwind() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let runnables = journal::foreground_runnable_counter();
        runnables.queued();
        let outer = task_timing(
            SpawnTime(Instant::now() - Duration::from_secs(2)),
            std::panic::Location::caller(),
        );
        let enclosing = THREAD_TIMINGS.with(|timings| {
            let mut timings = timings.lock();
            let running = timings.running.as_mut().unwrap();
            running.start = Instant::now() - Duration::from_millis(1);
            *running
        });
        runnables.queued();
        let result = catch_unwind(|| {
            let _inner = task_timing(SpawnTime(Instant::now()), std::panic::Location::caller());
            THREAD_TIMINGS.with(|timings| {
                timings.lock().running.as_mut().unwrap().start =
                    Instant::now() - Duration::from_millis(1);
            });
            panic!("nested poll unwind");
        });
        assert!(result.is_err());
        let restored = THREAD_TIMINGS.with(|timings| timings.lock().running.unwrap());
        assert_eq!(restored.start, enclosing.start);
        assert_eq!(restored.spawned.0, enclosing.spawned.0);
        assert_eq!(restored.location, enclosing.location);
        assert!(!has_idle(&collector.collect_unseen().entries));
        drop(outer);
        assert!(has_idle(&collector.collect_unseen().entries));
        assert!(THREAD_TIMINGS.with(|timings| timings.lock().running.is_none()));
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn unqueued_nested_poll_unwind_preserves_the_outer_running_task_and_queue() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        journal::foreground_runnable_counter().queued();
        let outer = task_timing(SpawnTime(Instant::now()), std::panic::Location::caller());
        let enclosing = THREAD_TIMINGS.with(|timings| {
            let mut timings = timings.lock();
            let running = timings.running.as_mut().unwrap();
            running.start = Instant::now() - Duration::from_millis(1);
            *running
        });
        let result = catch_unwind(|| {
            let _inner =
                task_timing_unqueued(SpawnTime(Instant::now()), std::panic::Location::caller());
            THREAD_TIMINGS.with(|timings| {
                timings.lock().running.as_mut().unwrap().start =
                    Instant::now() - Duration::from_millis(1);
            });
            panic!("unqueued nested poll unwind");
        });
        assert!(result.is_err());
        let restored = THREAD_TIMINGS.with(|timings| timings.lock().running.unwrap());
        assert_eq!(restored.start, enclosing.start);
        assert_eq!(restored.spawned.0, enclosing.spawned.0);
        assert!(!has_idle(&collector.collect_unseen().entries));
        drop(outer);
        assert!(has_idle(&collector.collect_unseen().entries));
        assert!(THREAD_TIMINGS.with(|timings| timings.lock().running.is_none()));
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn input_kind_and_invalidation_are_journaled_even_when_latency_is_reset() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF001);
        profiler.begin_input("MouseMove");
        profiler.end_input(false);
        profiler.begin_input("KeyDown");
        profiler.reset_input_latency_measurements();
        profiler.end_input_dispatch(0, 1, true);
        let inputs = collector
            .collect_unseen()
            .entries
            .into_iter()
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Input(timing)) => Some(timing),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].kind, "MouseMove");
        assert!(!inputs[0].caused_invalidation);
        assert_eq!(inputs[1].kind, "KeyDown");
        assert!(inputs[1].caused_invalidation);
        assert!(inputs.iter().all(|timing| timing.start <= timing.end));
        assert_eq!(profiler.pending_input_count, 0);
        assert!(profiler.input_latency_histogram.is_empty());
        drop(profiler);
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn only_submitted_outcomes_publish_the_exact_accepted_frame() {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF002);
        let base = Instant::now() - Duration::from_secs(1);
        input_at(&mut profiler, base, "MouseDown");
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        let accepted = profiler.pending_frame.unwrap();
        profiler.begin_input("KeyDown");
        profiler.end_input(true);
        collector.collect_unseen();
        for outcome in [
            PlatformWindowPresentOutcome::Deferred,
            PlatformWindowPresentOutcome::RepaintRequired,
            PlatformWindowPresentOutcome::Rejected,
        ] {
            present(&mut profiler, outcome);
            let entries = collector.collect_unseen().entries;
            assert!(!entries.iter().any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
            )));
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| matches!(
                        entry,
                        ForegroundJournalEntry::Event(ForegroundEvent::Present(timing))
                            if timing.window_id == profiler.window_id
                                && timing.animation_interval.is_none()
                    ))
                    .count(),
                1
            );
            assert_eq!(profiler.pending_input_count, 2);
            assert_eq!(
                profiler.pending_frame.unwrap().timing.draw_end,
                accepted.timing.draw_end
            );
            assert!(profiler.input_latency_histogram.is_empty());
        }
        let start = Instant::now();
        let end = start + Duration::from_millis(2);
        {
            let _turn = journal::foreground_turn();
            profiler.record_present_outcome(
                PlatformWindowPresentOutcome::Submitted,
                start,
                end,
                true,
                true,
            );
        }
        let entries = collector.collect_unseen().entries;
        let presented = entries
            .iter()
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented)) => {
                    Some(presented)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(presented.len(), 1);
        assert_eq!(presented[0].frame.draw_start, accepted.timing.draw_start);
        assert_eq!(presented[0].frame.draw_end, accepted.timing.draw_end);
        assert_eq!(presented[0].frame.dirty_at, accepted.timing.dirty_at);
        assert_eq!(
            presented[0].frame.invalidations,
            accepted.timing.invalidations
        );
        assert_eq!(presented[0].presentation.present_start, start);
        assert_eq!(presented[0].presentation.present_end, end);
        assert_eq!(profiler.pending_input_count, 1);
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert_eq!(profiler.events_per_frame_histogram.max(), 1);
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Present(_))
        )));
        assert!(!entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));
        assert_eq!(profiler.pending_input_count, 1);
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
    }

    #[test]
    fn failed_submission_work_is_retained_without_completing_the_pending_frame() {
        for outcome in [
            PlatformWindowPresentOutcome::Deferred,
            PlatformWindowPresentOutcome::Rejected,
            PlatformWindowPresentOutcome::RepaintRequired,
        ] {
            let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
            let mut profiler = profiler(0xF00B);
            let start = Instant::now() - Duration::from_millis(200);
            draw_at(&mut profiler, start);
            let pending = profiler.pending_frame.unwrap();
            let mut detector =
                hang::HangDetector::new(journal.clone(), Duration::from_millis(50), Duration::MAX);
            let mut collector = journal.collector();
            {
                let _turn = journal::foreground_turn();
                profiler.record_present_outcome(
                    outcome,
                    start,
                    start + Duration::from_millis(100),
                    true,
                    false,
                );
            }
            let entries = collector.collect_unseen().entries;
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| matches!(
                        entry,
                        ForegroundJournalEntry::Event(ForegroundEvent::Present(timing))
                            if timing.present_start == start
                                && timing.present_duration() == Duration::from_millis(100)
                    ))
                    .count(),
                1,
            );
            assert!(!entries.iter().any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
            )));
            assert_eq!(
                profiler.pending_frame.unwrap().timing.draw_start,
                pending.timing.draw_start
            );
            assert_eq!(profiler.submitted_present_count, 0);
            assert!(profiler.input_latency_histogram.is_empty());
            assert!(profiler.dirty_to_present_histogram.is_empty());
            drop(profiler);
            assert_idle_probe(&mut collector);
            let incidents = detector.poll();
            assert!(
                incidents
                    .iter()
                    .any(
                        |incident| incident.snapshot.events.iter().any(|event| matches!(
                            event,
                            ForegroundEvent::Present(timing)
                                if timing.present_start == start
                                    && timing.present_duration() == Duration::from_millis(100)
                        ))
                    )
            );
        }
    }

    #[test]
    fn draw_checkpoint_rollback_balances_nested_scopes_and_preserves_enclosing_input() {
        for unwind in [false, true] {
            let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
            let mut collector = journal.collector();
            let mut profiler = profiler(0xF003);
            draw_at(&mut profiler, Instant::now() - Duration::from_secs(1));
            let accepted = profiler.pending_frame.unwrap().timing;
            collector.collect_unseen();
            profiler.begin_input("outer");
            let checkpoint = profiler.checkpoint_draw();
            let result = catch_unwind(AssertUnwindSafe(|| {
                profiler.begin_draw();
                profiler.begin_input("unfinished_inner");
                let _action = ActionHandlerGuard::new("candidate_action");
                if unwind {
                    panic!("candidate draw unwind");
                }
            }));
            if unwind {
                assert!(result.is_err());
                profiler.abort_draw_checkpoint(checkpoint);
            } else {
                assert!(result.is_ok());
                assert_eq!(
                    profiler.finish_draw_checkpoint(checkpoint, None, 1, false),
                    Duration::ZERO
                );
            }
            assert_eq!(profiler.active_activities.len(), 1);
            assert_eq!(
                profiler.pending_frame.unwrap().timing.draw_end,
                accepted.draw_end
            );
            let entries = collector.collect_unseen().entries;
            assert!(!has_idle(&entries));
            assert!(!entries.iter().any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(_))
                    | ForegroundJournalEntry::Event(ForegroundEvent::Input(_))
            )));
            journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Hidden);
            {
                let _nested = journal::foreground_turn();
                let now = Instant::now();
                journal::record_input(journal::InputTiming {
                    kind: "nested_probe",
                    start: now,
                    end: now,
                    caused_invalidation: false,
                });
            }
            assert!(!has_idle(&collector.collect_unseen().entries));
            profiler.end_input_dispatch(0, 1, false);
            let entries = collector.collect_unseen().entries;
            assert!(has_idle(&entries), "{entries:?}");
            assert!(profiler.active_activities.is_empty());
            assert_idle_probe(&mut collector);
        }
    }

    #[test]
    fn one_window_rollback_does_not_end_another_windows_input_turn() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let mut outer = profiler(0xF004);
        let mut inner = profiler(0xF005);
        let checkpoint = outer.checkpoint_draw();
        outer.begin_draw();
        inner.begin_input("other_window");
        outer.abort_draw_checkpoint(checkpoint);
        journal::record_window_visibility(outer.window_id, crate::WindowVisibility::Hidden);
        journal::record_window_visibility(inner.window_id, crate::WindowVisibility::Hidden);
        assert!(!has_idle(&collector.collect_unseen().entries));
        inner.end_input(false);
        assert!(has_idle(&collector.collect_unseen().entries));
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn reset_during_rollback_preserves_frame_metadata_without_restoring_input_samples() {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF006);
        let base = Instant::now() - Duration::from_secs(1);
        input_at(&mut profiler, base, "covered_input");
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        let accepted = profiler.pending_frame.unwrap().timing;
        collector.collect_unseen();
        profiler.begin_input("reset_input");
        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        profiler.begin_input("discarded_input");
        profiler.reset_input_latency_measurements();
        profiler.abort_draw_checkpoint(checkpoint);
        profiler.end_input(true);
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.pending_frame.unwrap().input_count, 0);
        assert_eq!(profiler.pending_frame.unwrap().first_input_at, None);
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented))
                if presented.frame.draw_start == accepted.draw_start
                    && presented.frame.draw_end == accepted.draw_end
        )));
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.events_per_frame_histogram.is_empty());
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn accepted_publication_unwind_cannot_attach_an_older_frame_to_a_submission() {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF007);
        let base = Instant::now() - Duration::from_secs(1);
        input_at(&mut profiler, base, "first_input");
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        profiler.begin_input("late_input");
        profiler.end_input(true);
        collector.collect_unseen();
        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        profiler.abort_draw_checkpoint(checkpoint);
        profiler.discard_pending_frame();
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Present(_))
        )));
        assert!(!entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));
        assert_eq!(profiler.pending_input_count, 2);
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.dirty_to_present_histogram.is_empty());
        profiler.begin_draw();
        profiler.end_draw(Some(Instant::now()), 1);
        let recovered = profiler.pending_frame.unwrap().timing;
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented))
                if presented.frame.draw_start == recovered.draw_start
                    && presented.frame.draw_end == recovered.draw_end
        )));
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.events_per_frame_histogram.max(), 2);
    }

    #[test]
    fn interruptions_drop_latency_samples_but_hiding_keeps_draw_work() {
        for sleep in [false, true] {
            let (_journal, _guard) = journal::install_test_foreground_journal(64, 4);
            let id = WindowId::from(1);
            let mut profiler = WindowProfiler::new(id).expect("valid histograms");
            let old = Instant::now() - Duration::from_secs(1);
            draw_at(&mut profiler, old);
            profiler.record_present(old, old, true, true);
            profiler.first_input_at = Some(old);
            profiler.pending_input_count = 1;
            profiler.begin_draw();
            if sleep {
                journal::record_power_transition(journal::PowerState::Suspended);
                journal::record_power_transition(journal::PowerState::Awake);
            } else {
                journal::record_window_visibility(id, crate::WindowVisibility::Hidden);
                journal::record_window_visibility(id, crate::WindowVisibility::Visible);
            }
            profiler.end_draw(Some(old), 1);
            let now = Instant::now();
            profiler.record_present(now, now, true, true);
            assert_eq!(
                profiler.draw_duration_histogram.len(),
                if sleep { 1 } else { 2 }
            );
            assert_eq!(profiler.input_latency_histogram.len(), 0);
            assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
            assert_eq!(profiler.present_interval_histogram.len(), 0);
            profiler.begin_draw();
            profiler.end_draw(Some(Instant::now()), 1);
            let now = Instant::now();
            profiler.record_present(now, now, true, true);
            assert_eq!(profiler.dirty_to_present_histogram.len(), 2);
        }
    }

    #[test]
    fn unnotified_suspend_discards_input_action_draw_and_task_measurements() {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF008);
        draw_at(&mut profiler, Instant::now() - Duration::from_secs(1));
        collector.collect_unseen();
        profiler.begin_input("suspended_input");
        profiler.begin_draw();
        let action = ActionHandlerGuard::new("suspended_action");
        journal::foreground_runnable_counter().queued();
        let task = task_timing(SpawnTime(Instant::now()), std::panic::Location::caller());
        journal::advance_test_suspended_time(Duration::from_secs(2));
        drop(task);
        drop(action);
        profiler.end_draw(Some(Instant::now()), 1);
        profiler.end_input(true);
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::PowerTransition { .. })
        )));
        assert!(!entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(
                ForegroundEvent::TaskPoll(_)
                    | ForegroundEvent::Action(_)
                    | ForegroundEvent::Input(_)
                    | ForegroundEvent::Draw(_)
            )
        )));
        assert!(profiler.pending_frame.is_none());
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.draw_duration_histogram.len(), 1);
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn stale_pending_frame_cannot_complete_input_received_after_visibility_recovers() {
        let (journal, _guard) = journal::install_test_foreground_journal(128, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF009);
        let base = Instant::now() - Duration::from_secs(1);
        input_at(&mut profiler, base, "stale_input");
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Hidden);
        journal::record_window_visibility(profiler.window_id, crate::WindowVisibility::Visible);
        profiler.begin_input("fresh_input");
        profiler.end_input(true);
        assert_eq!(profiler.pending_input_count, 1);
        collector.collect_unseen();
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        let entries = collector.collect_unseen().entries;
        assert!(!entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));
        assert_eq!(profiler.pending_input_count, 1);
        assert!(profiler.input_latency_histogram.is_empty());
        profiler.begin_draw();
        profiler.end_draw(Some(Instant::now()), 1);
        present(&mut profiler, PlatformWindowPresentOutcome::Submitted);
        assert_eq!(profiler.pending_input_count, 0);
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert_eq!(profiler.events_per_frame_histogram.max(), 1);
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
    }

    #[test]
    fn nested_action_guards_keep_exact_names_and_balance_turns_during_unwind() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let outer = ActionHandlerGuard::new("outer_action");
        let result = catch_unwind(|| {
            let _inner = ActionHandlerGuard::new("inner_action");
            panic!("action handler unwind");
        });
        assert!(result.is_err());
        let inner_entries = collector.collect_unseen().entries;
        assert!(!has_idle(&inner_entries));
        drop(outer);
        let outer_entries = collector.collect_unseen().entries;
        assert!(has_idle(&outer_entries));
        let actions = inner_entries
            .into_iter()
            .chain(outer_entries)
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Action(timing)) => Some(timing),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].name, "inner_action");
        assert_eq!(actions[1].name, "outer_action");
        assert!(actions[1].start <= actions[0].start);
        assert!(actions[0].end <= actions[1].end);
        assert_idle_probe(&mut collector);
    }

    #[test]
    fn dropping_a_window_closes_its_journal_state_and_unfinished_activity_turns() {
        let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
        let mut collector = journal.collector();
        let mut profiler = profiler(0xF00A);
        let window_id = profiler.window_id;
        profiler.begin_input("outer_input");
        profiler.begin_draw();
        profiler.begin_input("inner_input");
        drop(profiler);
        assert!(
            collector
                .collect_unseen()
                .entries
                .iter()
                .any(|entry| matches!(
                    entry,
                    ForegroundJournalEntry::FrameState(FrameStateChange::Closed {
                        window_id: id,
                        ..
                    }) if *id == window_id
                ))
        );
        assert_idle_probe(&mut collector);
    }
}
