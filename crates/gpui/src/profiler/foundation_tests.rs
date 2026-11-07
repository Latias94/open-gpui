use super::*;

#[cfg(not(feature = "profiler"))]
#[test]
fn disabled_instrumentation_does_not_register_or_collect_timings() {
    std::thread::spawn(|| {
        let thread_id = std::thread::current().id();
        let now = Instant::now();
        assert!(!set_trace_enabled(true));
        assert!(!trace_enabled());

        update_running_task(SpawnTime(now), std::panic::Location::caller());
        save_task_timing();
        assert!(get_all_timings(TasksIncluded::CompletedAndRunning).is_empty());
        assert!(take_all_stats(TasksIncluded::CompletedAndRunning).is_empty());
        let current = get_current_thread_timings(TasksIncluded::CompletedAndRunning);
        assert!(current.timings.is_empty());
        assert_eq!(current.total_pushed, 0);
        assert!(
            GLOBAL_THREAD_TIMINGS
                .lock()
                .iter()
                .all(|thread| thread.thread_id != thread_id)
        );

        let mut timings = ThreadTimings::new(None, thread_id);
        timings.update_running_task(SpawnTime(now), std::panic::Location::caller());
        timings.save_task_timing(YieldTime(now + Duration::from_secs(1)));
        assert!(timings.running.is_none());
        assert!(timings.timings.is_empty());
        assert_eq!(timings.timings.capacity(), 0);

        let mut actions = ActionStatistics::default();
        actions.update_running_action("disabled", now);
        actions.save_action_timing();
        assert!(actions.is_empty());
        assert_eq!(actions.longest_runtimes(true).count(), 0);
        assert!(take_action_stats().is_empty());
        assert!(!trace_enabled());
    })
    .join()
    .unwrap();
}

#[test]
fn serialized_task_interfaces_preserve_origin_and_thread_identity() {
    let base = Instant::now();
    let timing = TaskTiming {
        location: std::panic::Location::caller(),
        spawned: SpawnTime(base),
        start: base + Duration::from_millis(1),
        end: YieldTime(base + Duration::from_millis(3)),
    };
    let thread_id = std::thread::current().id();
    let thread = ThreadTaskTimings {
        thread_name: Some("foundation".into()),
        thread_id,
        timings: vec![timing],
        stats: TaskStatistics::default(),
        total_pushed: 1,
    };
    let serialized = SerializedThreadTaskTimings::convert(base, thread.clone());
    let json = serde_json::to_string(&serialized).unwrap();
    let restored: SerializedThreadTaskTimings = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.thread_name.as_deref(), Some("foundation"));
    assert_eq!(restored.thread_id, serialized.thread_id);
    assert_eq!(restored.timings.len(), 1);
    assert_eq!(restored.timings[0].start, 1_000_000);
    assert_eq!(restored.timings[0].duration, 2_000_000);
    assert_eq!(restored.timings[0].location.line, timing.location.line());
    assert_eq!(
        restored.timings[0].location.column,
        timing.location.column()
    );

    let mut collector = ProfilingCollector::new(base);
    let first = collector.collect_unseen(vec![thread.clone()]);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].thread_id, serialized.thread_id);
    assert_eq!(first[0].new_timings.len(), 1);
    assert!(collector.collect_unseen(vec![thread.clone()]).is_empty());
    collector.reset();
    assert_eq!(collector.collect_unseen(vec![thread]).len(), 1);
}

#[cfg(feature = "profiler")]
mod enabled {
    use super::*;
    use std::sync::{Mutex, MutexGuard, mpsc};

    static TRACE_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TraceTestGuard {
        was_enabled: bool,
        _lock: MutexGuard<'static, ()>,
    }

    impl TraceTestGuard {
        fn new() -> Self {
            let lock = TRACE_TEST_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let was_enabled = TRACE_STATE.load(Ordering::Acquire) & TRACE_SETTING_ENABLED != 0;
            set_trace_enabled(false);
            Self {
                was_enabled,
                _lock: lock,
            }
        }
    }

    impl Drop for TraceTestGuard {
        fn drop(&mut self) {
            set_trace_enabled(self.was_enabled);
        }
    }

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

    fn input_at(profiler: &mut WindowProfiler, started_at: Instant) {
        profiler.active_activities.push(WindowActivity::Input {
            started_at,
            kind: "test_input",
            covering_draw: None,
            measurement_generation: profiler.input_measurement_generation,
            _input_activity: journal::input_activity("test_input", started_at),
            _turn: journal::foreground_turn(),
        });
    }

    fn matches_window(event: &FrameEvent, window_id: WindowId) -> bool {
        match event {
            FrameEvent::Draw(frame) => frame.window_id == window_id,
            FrameEvent::Present(present) => present.window_id == window_id,
        }
    }

    fn assert_collection_releases_registry_before_locking_threads() {
        for operation in [0, 1, 2] {
            set_trace_enabled(true);
            let (ready, receive_ready) = mpsc::sync_channel(0);
            let (release, receive_release) = mpsc::sync_channel(0);
            let worker = std::thread::spawn(move || {
                update_running_task(SpawnTime(Instant::now()), std::panic::Location::caller());
                save_task_timing();
                THREAD_TIMINGS.with(|timings| {
                    let locked = timings.lock();
                    ready.send(Arc::downgrade(timings)).unwrap();
                    receive_release.recv().unwrap();
                    drop(locked);
                });
            });
            let weak = receive_ready.recv().unwrap();
            let pinned_owner = weak.upgrade().unwrap();
            let collector = std::thread::spawn(move || match operation {
                0 => {
                    get_all_timings(TasksIncluded::OnlyCompleted);
                }
                1 => {
                    take_all_stats(TasksIncluded::OnlyCompleted);
                }
                _ => {
                    set_trace_enabled(false);
                }
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while weak.strong_count() < 3 && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            let upgraded = weak.strong_count() >= 3;
            let mut registry_released = false;
            while upgraded && std::time::Instant::now() < deadline {
                if GLOBAL_THREAD_TIMINGS.try_lock().is_some() {
                    registry_released = true;
                    break;
                }
                std::thread::yield_now();
            }

            release.send(()).unwrap();
            worker.join().unwrap();
            collector.join().unwrap();
            drop(pinned_owner);
            assert!(upgraded, "collector must upgrade the blocked thread owner");
            assert!(
                registry_released,
                "operation {operation} held the registry while waiting for a thread buffer"
            );
            set_trace_enabled(false);
        }
    }

    #[test]
    fn collectors_release_last_thread_owners_after_unlocking_the_registry() {
        let _trace = TraceTestGuard::new();
        assert_collection_releases_registry_before_locking_threads();
        for collect_statistics in [false, true] {
            let (ready, receive_ready) = mpsc::sync_channel(0);
            let (exit, receive_exit) = mpsc::sync_channel(0);
            let worker = std::thread::spawn(move || {
                update_running_task(SpawnTime(Instant::now()), std::panic::Location::caller());
                save_task_timing();
                ready.send(std::thread::current().id()).unwrap();
                receive_exit.recv().unwrap();
            });
            let thread_id = receive_ready.recv().unwrap();
            let mut owners = upgraded_thread_timings();
            assert!(GLOBAL_THREAD_TIMINGS.try_lock().is_some());
            let index = owners.iter().position(|(id, _)| *id == thread_id).unwrap();
            let (_, owner) = owners.swap_remove(index);
            drop(owners);
            let weak = Arc::downgrade(&owner);
            exit.send(()).unwrap();
            worker.join().unwrap();
            assert_eq!(Arc::strong_count(&owner), 1);

            if collect_statistics {
                let collected = ThreadTaskStatistics::collect_and_reset(
                    vec![(thread_id, owner)],
                    TasksIncluded::OnlyCompleted,
                );
                assert_eq!(collected.len(), 1);
                assert_eq!(collected[0].thread_id, thread_id);
            } else {
                let collected = ThreadTaskTimings::collect(
                    vec![(thread_id, owner)],
                    TasksIncluded::OnlyCompleted,
                );
                assert_eq!(collected.len(), 1);
                assert_eq!(collected[0].thread_id, thread_id);
            }
            assert!(weak.upgrade().is_none());
            assert!(
                GLOBAL_THREAD_TIMINGS
                    .lock()
                    .iter()
                    .all(|thread| thread.thread_id != thread_id)
            );
        }
    }

    #[test]
    fn task_statistics_do_not_allocate_trace_storage_without_a_trace_scope() {
        let _trace = TraceTestGuard::new();
        let base = Instant::now();
        let mut timings = ThreadTimings::new(None, std::thread::current().id());
        timings.running = Some(ActiveTiming {
            location: std::panic::Location::caller(),
            spawned: SpawnTime(base),
            start: base,
        });
        timings.save_task_timing(YieldTime(base + Duration::from_millis(5)));
        assert!(timings.timings.is_empty());
        assert_eq!(timings.timings.capacity(), 0);
        assert_eq!(timings.total_pushed, 0);
        assert_eq!(
            timings.stats.longest_poll_times[0].poll_duration(),
            Duration::from_millis(5)
        );

        let scope = trace_scope();
        timings.running = Some(ActiveTiming {
            location: std::panic::Location::caller(),
            spawned: SpawnTime(base),
            start: base,
        });
        timings.save_task_timing(YieldTime(base + Duration::from_millis(6)));
        assert_eq!(timings.timings.len(), 1);
        assert_eq!(timings.total_pushed, 1);
        drop(scope);
    }

    #[test]
    fn longest_poll_statistics_rank_poll_time_independently_of_total_runtime() {
        let mut statistics = TaskStatistics::default();
        let base = Instant::now();
        for (runtime, poll) in [(100, 5), (90, 6), (80, 7), (70, 8), (60, 9), (1000, 1)] {
            statistics.add_yield_timing(TaskTiming {
                location: std::panic::Location::caller(),
                spawned: SpawnTime(base),
                start: base + Duration::from_millis(runtime - poll),
                end: YieldTime(base + Duration::from_millis(runtime)),
            });
        }
        let mut polls = statistics
            .longest_poll_times
            .map(|task| task.poll_duration());
        polls.sort();
        assert_eq!(polls, [5, 6, 7, 8, 9].map(Duration::from_millis));
    }

    #[test]
    fn trace_scopes_and_manual_setting_release_only_their_own_enablement() {
        let _trace = TraceTestGuard::new();
        let first = trace_scope();
        let second = trace_scope();
        assert!(trace_enabled());
        assert!(set_trace_enabled(true));
        drop(first);
        assert!(trace_enabled());
        assert!(set_trace_enabled(false));
        assert!(!set_trace_enabled(false));
        assert!(trace_enabled());
        drop(second);
        assert!(!trace_enabled());

        set_trace_enabled(true);
        let scope = trace_scope();
        drop(scope);
        assert!(trace_enabled());
        set_trace_enabled(false);
    }

    /// Covers scope and manual trace restarts while prior cleanup is blocked.
    /// The lifecycle lock probe only selects a deadlock-free buffer release order;
    /// both schedules must retain the new consumer's event without timing-based detection.
    #[test]
    fn trace_reenable_waits_for_prior_cleanup_and_preserves_new_events() {
        let _trace = TraceTestGuard::new();
        for (disable_manually, enable_manually) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let initial_scope = if disable_manually {
                set_trace_enabled(true);
                None
            } else {
                Some(trace_scope())
            };
            let owner = THREAD_TIMINGS.with(|timings| Arc::clone(timings));
            let weak = Arc::downgrade(&owner);
            let owners_before_cleanup = weak.strong_count();
            let locked = owner.lock();
            let clearing = std::thread::spawn(move || {
                if disable_manually {
                    set_trace_enabled(false);
                } else {
                    drop(initial_scope);
                }
            });

            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while weak.strong_count() <= owners_before_cleanup
                && std::time::Instant::now() < deadline
            {
                std::thread::yield_now();
            }
            let cleanup_reached_thread = weak.strong_count() > owners_before_cleanup;

            let cleanup_serialized = TRACE_LIFECYCLE_LOCK.try_lock().is_none();
            let (attempting, receive_attempting) = mpsc::sync_channel(0);
            let (recorded, receive_recorded) = mpsc::sync_channel(0);
            let resuming = std::thread::spawn(move || {
                attempting.send(()).unwrap();
                let scope = if enable_manually {
                    set_trace_enabled(true);
                    None
                } else {
                    Some(trace_scope())
                };
                let collector = FrameTimingCollector::new();
                let mut profiler = profiler(0xC1E4);
                profiler.begin_draw();
                profiler.end_draw(None, 0);
                recorded.send(()).unwrap();
                (scope, collector, profiler.window_id)
            });
            receive_attempting.recv().unwrap();
            let recorded = if cleanup_serialized {
                drop(locked);
                receive_recorded.recv_timeout(Duration::from_secs(2))
            } else {
                let recorded = receive_recorded.recv_timeout(Duration::from_secs(2));
                drop(locked);
                recorded
            };
            clearing.join().unwrap();
            let (scope, mut collector, window_id) = resuming.join().unwrap();
            let events = collector.collect_unseen();
            drop(scope);
            set_trace_enabled(false);

            assert!(
                cleanup_reached_thread,
                "cleanup must reach the blocked thread buffer"
            );
            recorded.expect("the new trace must finish recording after cleanup is released");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches_window(event, window_id))
                    .count(),
                1,
                "new trace event lost: disable_manually={disable_manually}, enable_manually={enable_manually}"
            );
        }
    }

    #[test]
    fn trace_collectors_continue_after_buffers_are_cleared() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0xC1EA);
        let base = Instant::now();
        let mut frames = FrameTimingCollector::new();
        let mut tasks = ProfilingCollector::new(base);
        let scope = trace_scope();
        draw_at(&mut profiler, base);
        update_running_task(SpawnTime(base), std::panic::Location::caller());
        save_task_timing();
        assert_eq!(
            frames
                .collect_unseen()
                .iter()
                .filter(|event| matches_window(event, profiler.window_id))
                .count(),
            1
        );
        assert!(
            !tasks
                .collect_unseen(get_all_timings(TasksIncluded::OnlyCompleted))
                .is_empty()
        );
        drop(scope);
        assert!(FRAME_TIMINGS.lock().timings.is_empty());
        assert!(frames.collect_unseen().is_empty());

        let scope = trace_scope();
        draw_at(&mut profiler, base + Duration::from_millis(10));
        update_running_task(SpawnTime(base), std::panic::Location::caller());
        save_task_timing();
        assert_eq!(
            frames
                .collect_unseen()
                .iter()
                .filter(|event| matches_window(event, profiler.window_id))
                .count(),
            1
        );
        let deltas = tasks.collect_unseen(get_all_timings(TasksIncluded::OnlyCompleted));
        assert!(deltas.iter().any(|delta| delta.new_timings.len() == 1));
        drop(scope);
    }

    #[test]
    fn window_aggregates_remain_enabled_without_retaining_frame_events() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0xD0A0);
        let mut collector = FrameTimingCollector::new();
        profiler.begin_input("test");
        profiler.end_input(true);
        profiler.begin_draw();
        let duration = profiler.end_draw(None, 2);
        let present_start = Instant::now();
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            present_start,
            present_start + Duration::from_millis(3),
            true,
            false,
        );
        let input = profiler.input_latency_snapshot();
        assert_eq!(input.latency_histogram.len(), 1);
        assert_eq!(input.events_per_frame_histogram.max(), 1);
        assert_eq!(input.drawn_frame_count, 1);
        let frame = profiler.frame_duration_snapshot();
        assert_eq!(frame.draw_duration_histogram.len(), 1);
        assert!(frame.draw_duration_histogram.max() >= duration.as_nanos() as u64);
        assert!(
            collector
                .collect_unseen()
                .iter()
                .all(|event| !matches_window(event, profiler.window_id))
        );
    }

    #[test]
    fn nested_inputs_use_the_earliest_invalidating_start_and_exclude_mid_draw_input() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0x1A7E);
        let base = Instant::now();
        input_at(&mut profiler, base);
        input_at(&mut profiler, base + Duration::from_millis(2));
        profiler.end_input(true);
        profiler.end_input(true);
        input_at(&mut profiler, base + Duration::from_millis(3));
        profiler.end_input(false);
        profiler.begin_draw();
        profiler.begin_input("mid_draw");
        profiler.end_input(true);
        profiler.end_draw(Some(base), 3);
        profiler.record_present(
            base + Duration::from_millis(8),
            base + Duration::from_millis(12),
            true,
            false,
        );
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.latency_histogram.len(), 1);
        assert!(snapshot.latency_histogram.max() >= 12_000_000);
        assert_eq!(snapshot.events_per_frame_histogram.max(), 2);
        assert_eq!(snapshot.pending_input_count, 0);
        assert_eq!(snapshot.mid_draw_events_dropped, 1);
    }

    #[test]
    fn only_submitted_outcomes_flush_input_and_new_frame_timings() {
        let _trace = TraceTestGuard::new();
        let scope = trace_scope();
        let mut profiler = profiler(0xACCE);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        let mut collector = FrameTimingCollector::new();
        for outcome in [
            PlatformWindowPresentOutcome::Deferred,
            PlatformWindowPresentOutcome::RepaintRequired,
            PlatformWindowPresentOutcome::Rejected,
        ] {
            profiler.record_present_outcome(
                outcome,
                base + Duration::from_millis(5),
                base + Duration::from_millis(6),
                true,
                true,
            );
            let snapshot = profiler.input_latency_snapshot();
            assert_eq!(snapshot.pending_input_count, 1);
            assert!(snapshot.latency_histogram.is_empty());
            assert!(profiler.pending_frame.is_some());
        }
        assert!(collector.collect_unseen().is_empty());
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(10),
            base + Duration::from_millis(12),
            true,
            true,
        );
        let input = profiler.input_latency_snapshot();
        assert_eq!(input.present_attempt_count, 4);
        assert_eq!(input.submitted_present_count, 1);
        assert_eq!(input.deferred_present_count, 1);
        assert_eq!(input.repaint_required_present_count, 1);
        assert_eq!(input.rejected_present_count, 1);
        assert_eq!(input.pending_input_count, 0);
        assert!(input.latency_histogram.max() >= 12_000_000);
        let frame = profiler.frame_duration_snapshot();
        assert_eq!(frame.draw_duration_histogram.len(), 1);
        assert_eq!(frame.dirty_to_present_histogram.len(), 1);
        assert!(frame.dirty_to_present_histogram.max() >= 12_000_000);
        let presents = collector
            .collect_unseen()
            .into_iter()
            .filter_map(|event| match event {
                FrameEvent::Present(present) if present.window_id == profiler.window_id => {
                    Some(present)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(presents.len(), 1);
        assert_eq!(presents[0].present_start, base + Duration::from_millis(10));
        assert_eq!(presents[0].present_end, base + Duration::from_millis(12));
        assert_eq!(presents[0].present_duration(), Duration::from_millis(2));
        drop(scope);
    }

    #[test]
    fn submitting_an_older_draw_leaves_later_input_for_its_own_frame() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0x1A7F);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Deferred,
            base + Duration::from_millis(5),
            base + Duration::from_millis(6),
            true,
            false,
        );
        input_at(&mut profiler, base + Duration::from_millis(7));
        profiler.end_input(true);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(8),
            base + Duration::from_millis(10),
            true,
            false,
        );
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.latency_histogram.len(), 1);
        assert!(snapshot.latency_histogram.max() >= 10_000_000);
        assert_eq!(snapshot.events_per_frame_histogram.max(), 1);
        assert_eq!(snapshot.pending_input_count, 1);

        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(11),
            base + Duration::from_millis(12),
            true,
            false,
        );
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert_eq!(profiler.input_latency_snapshot().pending_input_count, 1);
        draw_at(&mut profiler, base + Duration::from_millis(13));
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(18),
            base + Duration::from_millis(20),
            true,
            false,
        );
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.latency_histogram.len(), 2);
        assert!(snapshot.latency_histogram.max() >= 13_000_000);
        assert!(snapshot.latency_histogram.max() < 14_000_000);
        assert_eq!(snapshot.events_per_frame_histogram.max(), 1);
        assert_eq!(snapshot.pending_input_count, 0);
    }

    #[test]
    fn a_superseding_accepted_draw_covers_all_unsubmitted_inputs() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0x5A9E);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        input_at(&mut profiler, base + Duration::from_millis(7));
        profiler.end_input(true);
        draw_at(&mut profiler, base + Duration::from_millis(8));
        profiler.record_present(
            base + Duration::from_millis(13),
            base + Duration::from_millis(14),
            true,
            false,
        );
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.latency_histogram.len(), 1);
        assert!(snapshot.latency_histogram.max() >= 14_000_000);
        assert_eq!(snapshot.events_per_frame_histogram.max(), 2);
        assert_eq!(snapshot.pending_input_count, 0);
        assert_eq!(profiler.draw_duration_histogram.len(), 2);
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
    }

    #[test]
    fn accepted_then_unwound_draw_cannot_complete_input_or_present_an_older_measurement() {
        let _trace = TraceTestGuard::new();
        let scope = trace_scope();
        let mut profiler = profiler(0xAB07);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        input_at(&mut profiler, base + Duration::from_millis(7));
        profiler.end_input(true);
        let mut collector = FrameTimingCollector::new();
        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        profiler.abort_draw_checkpoint(checkpoint);
        profiler.discard_pending_frame();
        assert!(profiler.active_activities.is_empty());
        assert!(profiler.pending_frame.is_none());
        assert_eq!(profiler.input_latency_snapshot().pending_input_count, 2);

        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(8),
            base + Duration::from_millis(10),
            true,
            false,
        );
        assert!(profiler.input_latency_histogram.is_empty());
        assert!(profiler.dirty_to_present_histogram.is_empty());
        assert_eq!(profiler.input_latency_snapshot().pending_input_count, 2);
        assert!(collector.collect_unseen().is_empty());

        draw_at(&mut profiler, base + Duration::from_millis(11));
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(16),
            base + Duration::from_millis(18),
            true,
            false,
        );
        assert_eq!(profiler.input_latency_histogram.len(), 1);
        assert!(profiler.input_latency_histogram.max() >= 18_000_000);
        assert_eq!(profiler.events_per_frame_histogram.max(), 2);
        assert_eq!(profiler.input_latency_snapshot().pending_input_count, 0);
        assert_eq!(
            collector
                .collect_unseen()
                .iter()
                .filter(|event| matches_window(event, profiler.window_id))
                .count(),
            2
        );
        drop(scope);
    }

    #[test]
    fn animation_intervals_require_new_draws_active_windows_and_scheduled_continuation() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0xF12A);
        let base = Instant::now();
        for (millis, active, next, count) in [
            (0, true, true, 0),
            (16, true, false, 1),
            (160, true, true, 1),
            (240, true, true, 2),
            (256, false, true, 2),
            (272, true, true, 2),
            (288, true, true, 3),
        ] {
            let start = base + Duration::from_millis(millis);
            draw_at(&mut profiler, start);
            profiler.record_present(
                start + Duration::from_millis(5),
                start + Duration::from_millis(6),
                active,
                next,
            );
            profiler.record_present(
                start + Duration::from_millis(10),
                start + Duration::from_millis(11),
                active,
                next,
            );
            assert_eq!(profiler.present_interval_histogram.len(), count);
        }
        assert!(profiler.present_interval_histogram.max() >= 80_000_000);
        assert_eq!(profiler.dirty_to_present_histogram.len(), 7);
    }

    #[test]
    fn draw_checkpoints_restore_deferred_frames_on_refusal_and_unwind() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0xDEF0);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Deferred,
            base + Duration::from_millis(5),
            base + Duration::from_millis(6),
            true,
            false,
        );
        let previous = profiler.pending_frame.unwrap();

        input_at(&mut profiler, base + Duration::from_millis(7));
        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        assert_eq!(
            profiler.finish_draw_checkpoint(checkpoint, Some(base), 9, false),
            Duration::ZERO
        );
        assert_eq!(profiler.active_activities.len(), 1);
        assert_eq!(
            profiler.pending_frame.unwrap().timing.draw_end,
            previous.timing.draw_end
        );

        let checkpoint = profiler.checkpoint_draw();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            profiler.begin_draw();
            profiler.begin_input("unwinding_input");
            panic!("candidate failed");
        }));
        assert!(result.is_err());
        profiler.abort_draw_checkpoint(checkpoint);
        assert_eq!(profiler.active_activities.len(), 1);
        assert_eq!(
            profiler.pending_frame.unwrap().timing.draw_start,
            previous.timing.draw_start
        );
        profiler.end_input(false);
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.drawn_frame_count, 1);
        assert_eq!(snapshot.pending_input_count, 1);
        assert_eq!(profiler.draw_duration_histogram.len(), 1);
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(10),
            base + Duration::from_millis(12),
            true,
            false,
        );
        assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
        assert!(profiler.dirty_to_present_histogram.max() >= 12_000_000);
        assert_eq!(profiler.input_latency_snapshot().pending_input_count, 0);
    }

    #[test]
    fn draw_rollback_does_not_restore_input_samples_cleared_by_reset() {
        let _trace = TraceTestGuard::new();
        let _scope = trace_scope();
        for abort in [false, true] {
            for resets in [1, 2] {
                let mut profiler = profiler(0xAE57);
                let base = Instant::now();
                input_at(&mut profiler, base);
                profiler.end_input(true);
                draw_at(&mut profiler, base);
                let accepted = profiler.pending_frame.unwrap().timing;
                let mut collector = FrameTimingCollector::new();

                let checkpoint = profiler.checkpoint_draw();
                profiler.begin_draw();
                for _ in 0..resets {
                    profiler.reset_input_latency_measurements();
                }
                if abort {
                    profiler.abort_draw_checkpoint(checkpoint);
                } else {
                    assert_eq!(
                        profiler.finish_draw_checkpoint(checkpoint, None, 0, false),
                        Duration::ZERO
                    );
                }

                assert!(profiler.active_activities.is_empty());
                let pending = profiler.pending_frame.unwrap();
                assert_eq!(pending.timing.draw_start, accepted.draw_start);
                assert_eq!(pending.timing.draw_end, accepted.draw_end);
                assert!(pending.first_input_at.is_none());
                assert_eq!(pending.input_count, 0);
                assert_eq!(profiler.pending_input_count, 0);

                input_at(&mut profiler, base + Duration::from_millis(7));
                profiler.end_input(true);
                profiler.record_present_outcome(
                    PlatformWindowPresentOutcome::Submitted,
                    base + Duration::from_millis(8),
                    base + Duration::from_millis(10),
                    true,
                    false,
                );
                let input = profiler.input_latency_snapshot();
                assert!(input.latency_histogram.is_empty());
                assert!(input.events_per_frame_histogram.is_empty());
                assert_eq!(input.pending_input_count, 1);
                assert_eq!(input.drawn_frame_count, 0);
                assert_eq!(input.submitted_present_count, 1);
                assert_eq!(profiler.draw_duration_histogram.len(), 1);
                assert_eq!(profiler.dirty_to_present_histogram.len(), 1);
                let events = collector.collect_unseen();
                assert!(matches!(events.as_slice(), [FrameEvent::Present(_)]));

                draw_at(&mut profiler, base + Duration::from_millis(11));
                profiler.record_present_outcome(
                    PlatformWindowPresentOutcome::Submitted,
                    base + Duration::from_millis(16),
                    base + Duration::from_millis(18),
                    true,
                    false,
                );
                let input = profiler.input_latency_snapshot();
                assert_eq!(input.pending_input_count, 0);
                assert_eq!(input.latency_histogram.len(), 1);
                assert!(input.latency_histogram.max() >= 11_000_000);
                assert!(input.latency_histogram.max() < 12_000_000);
                assert_eq!(input.events_per_frame_histogram.count_at(1), 1);
            }
        }
    }

    #[test]
    fn draw_checkpoints_finish_only_draws_started_inside_their_scope() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0x5C0E);
        profiler.begin_input("outer_input");
        profiler.begin_draw();
        let outer_start = match profiler.active_activities.last().unwrap() {
            WindowActivity::Draw { started_at, .. } => *started_at,
            _ => unreachable!(),
        };
        let checkpoint = profiler.checkpoint_draw();
        assert_eq!(
            profiler.finish_draw_checkpoint(checkpoint, None, 0, true),
            Duration::ZERO
        );
        assert_eq!(profiler.active_activities.len(), 2);
        assert!(profiler.pending_frame.is_none());

        let checkpoint = profiler.checkpoint_draw();
        profiler.begin_draw();
        profiler.finish_draw_checkpoint(checkpoint, Some(outer_start), 2, true);
        assert_eq!(profiler.active_activities.len(), 2);
        assert_eq!(profiler.input_latency_snapshot().drawn_frame_count, 1);
        profiler.abort_draw();
        assert_eq!(profiler.active_activities.len(), 1);
        assert!(profiler.pending_frame.is_some());
        profiler.end_input(false);
    }

    #[test]
    fn reset_clears_input_diagnostics_and_exact_callsites_without_losing_pending_draw() {
        let _trace = TraceTestGuard::new();
        let mut profiler = profiler(0x2E5E7);
        let base = Instant::now();
        input_at(&mut profiler, base);
        profiler.end_input(true);
        draw_at(&mut profiler, base);
        let first = std::panic::Location::caller();
        let second = std::panic::Location::caller();
        profiler.record_refresh_request(false, first);
        profiler.record_refresh_request(true, first);
        profiler.record_refresh_request(true, second);
        for outcome in [
            PlatformWindowPresentOutcome::Submitted,
            PlatformWindowPresentOutcome::Deferred,
            PlatformWindowPresentOutcome::RepaintRequired,
            PlatformWindowPresentOutcome::Rejected,
        ] {
            profiler.record_present_outcome(
                outcome,
                base + Duration::from_millis(8),
                base + Duration::from_millis(10),
                true,
                true,
            );
        }
        input_at(&mut profiler, base + Duration::from_millis(11));
        profiler.end_input(true);
        draw_at(&mut profiler, base + Duration::from_millis(12));
        profiler.begin_draw();
        profiler.begin_input("mid_draw");
        profiler.end_input(true);
        profiler.abort_draw();
        let snapshot = profiler.input_latency_snapshot();
        assert_eq!(snapshot.refresh_request_count, 3);
        assert_eq!(snapshot.coalesced_refresh_request_count, 2);
        assert_eq!(snapshot.refresh_callsites.len(), 2);
        assert_eq!(
            snapshot.refresh_callsites[0],
            InputLatencyRefreshCallsite {
                file: first.file(),
                line: first.line(),
                request_count: 2,
                coalesced_request_count: 1,
            }
        );
        assert_eq!(
            snapshot.refresh_callsites[1],
            InputLatencyRefreshCallsite {
                file: second.file(),
                line: second.line(),
                request_count: 1,
                coalesced_request_count: 1,
            }
        );
        assert_eq!(snapshot.pending_input_count, 1);
        assert_eq!(snapshot.mid_draw_events_dropped, 1);

        profiler.reset_input_latency_measurements();
        let snapshot = profiler.input_latency_snapshot();
        assert!(snapshot.latency_histogram.is_empty());
        assert!(snapshot.events_per_frame_histogram.is_empty());
        assert_eq!(snapshot.pending_input_count, 0);
        assert_eq!(snapshot.mid_draw_events_dropped, 0);
        assert_eq!(snapshot.drawn_frame_count, 0);
        assert_eq!(snapshot.present_attempt_count, 0);
        assert_eq!(snapshot.submitted_present_count, 0);
        assert_eq!(snapshot.deferred_present_count, 0);
        assert_eq!(snapshot.repaint_required_present_count, 0);
        assert_eq!(snapshot.rejected_present_count, 0);
        assert_eq!(snapshot.refresh_request_count, 0);
        assert_eq!(snapshot.coalesced_refresh_request_count, 0);
        assert!(snapshot.refresh_callsites.is_empty());
        assert_eq!(profiler.draw_duration_histogram.len(), 2);
        assert!(profiler.pending_frame.is_some());
        profiler.record_present_outcome(
            PlatformWindowPresentOutcome::Submitted,
            base + Duration::from_millis(30),
            base + Duration::from_millis(32),
            true,
            false,
        );
        assert!(profiler.input_latency_histogram.is_empty());
        assert_eq!(profiler.dirty_to_present_histogram.len(), 2);
        assert_eq!(profiler.present_interval_histogram.len(), 1);
    }
}
