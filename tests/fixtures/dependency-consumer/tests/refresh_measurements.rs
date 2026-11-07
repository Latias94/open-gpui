//! Ordinary dependency consumers: no test-support, application, or platform backend.
//! Synthetic timing values test arithmetic, never successful presentation receipts.

#[cfg(feature = "profiler")]
mod enabled {
    use open_gpui::{
        DrawPhases, FRAME_BUDGET_FLOOR, FRAME_DURATION_BUCKETS_MS, FRAME_WORK_BUCKETS_PERCENT,
        FrameDurationSnapshot, FrameTiming, PresentTiming, RefreshIntervalFrames, RunnableMeta,
        UNKNOWN_REFRESH_FRAME_BUDGET, ViewPhase, ViewTiming, Window, WindowId, WindowProfiler,
        histogram_bucket_counts,
        profiler::{
            self,
            hang::{
                HangDetector, HangIncident, HangTrigger, MEASUREMENT_VERSION,
                SerializedHangContributor, SerializedHangIncident,
            },
            journal::{ForegroundEvent, ForegroundJournalEntry, IntervalBoundary, IntervalSealer},
        },
    };
    use std::time::Duration;

    struct RestoreTrace(bool);

    impl Drop for RestoreTrace {
        fn drop(&mut self) {
            profiler::set_trace_enabled(self.0);
        }
    }

    #[test]
    fn downstream_v3_window_and_report_signatures_need_no_test_support() {
        let _: fn(&Window) -> FrameDurationSnapshot = Window::frame_duration_snapshot;
        let _: fn(&FrameTiming) -> Duration = FrameTiming::budget;
        let _: fn(&RefreshIntervalFrames, &RefreshIntervalFrames) -> RefreshIntervalFrames =
            RefreshIntervalFrames::since;

        let _window_type_only = |window: &Window, frame: FrameTiming, present: PresentTiming| {
            let FrameDurationSnapshot {
                dirty_to_present_histogram,
                draw_duration_histogram,
                present_interval_histogram,
                by_refresh_interval,
            } = window.frame_duration_snapshot();
            for frames in &by_refresh_interval {
                let RefreshIntervalFrames {
                    refresh_interval,
                    frames_drawn,
                    draw_duration,
                    frames_on_time,
                    frames_late,
                    missed_refreshes,
                    frames_unmeasured,
                    work,
                    input_frames_on_time,
                    input_frames_late,
                    input_work,
                } = frames;
                let _: Option<Duration> = *refresh_interval;
                let _: [u64; 8] = [
                    *frames_drawn,
                    *frames_on_time,
                    *frames_late,
                    *missed_refreshes,
                    *frames_unmeasured,
                    *input_frames_on_time,
                    *input_frames_late,
                    draw_duration.len(),
                ];
                let _: (u64, u64) = (work.len(), input_work.len());
            }
            let _: Option<Duration> = frame.refresh_interval;
            let _: DrawPhases = frame.phases;
            let _captured_signal = frame.signal_at;
            let _work_start = frame.work_start();
            let _: Option<Duration> = frame.work(present.present_start);
            let _: Option<bool> = frame.is_late(present.present_start);
            let _: Option<u64> = frame.missed_refreshes(present.present_start);
            (
                dirty_to_present_histogram,
                draw_duration_histogram,
                present_interval_histogram,
                by_refresh_interval,
            )
        };
        let _active_time_type_only = |detector: &mut HangDetector| {
            let now = RunnableMeta::new_with_callers_location().spawned.0;
            let _: Duration = detector.take_active_time(now, Duration::from_secs(1));
        };
    }

    #[test]
    fn downstream_frame_values_exclude_signal_wait_and_use_real_refresh_for_misses() {
        let start = RunnableMeta::new_with_callers_location().spawned.0;
        let mut frame = FrameTiming {
            window_id: WindowId::from(7),
            dirty_at: Some(start + Duration::from_millis(2)),
            invalidations: 3,
            draw_start: start + Duration::from_millis(6),
            draw_end: start + Duration::from_millis(7),
            refresh_interval: Some(Duration::from_millis(10)),
            signal_at: Some(start + Duration::from_millis(5)),
            phases: DrawPhases {
                request_layout: Duration::from_micros(400),
                prepaint: Duration::from_micros(300),
                paint: Duration::from_micros(200),
            },
        };
        let submitted_at = start + Duration::from_millis(15);
        assert_eq!(frame.work_start(), frame.signal_at);
        assert_eq!(frame.work(submitted_at), Some(Duration::from_millis(10)));
        assert_eq!(frame.budget(), Duration::from_millis(10));
        assert_eq!(frame.is_late(submitted_at), Some(false));
        assert_eq!(frame.missed_refreshes(submitted_at), Some(0));
        assert_eq!(
            frame.is_late(submitted_at + Duration::from_nanos(1)),
            Some(true)
        );
        assert_eq!(
            frame.missed_refreshes(submitted_at + Duration::from_nanos(1)),
            Some(1)
        );
        assert_eq!(frame.draw_duration(), Duration::from_millis(1));
        assert_eq!(
            frame.dirty_to_draw_duration(),
            Some(Duration::from_millis(5))
        );
        assert_eq!(frame.work(start), Some(Duration::ZERO));

        frame.dirty_at = Some(start + Duration::from_millis(8));
        assert_eq!(frame.work_start(), frame.dirty_at);
        assert_eq!(frame.work(submitted_at), Some(Duration::from_millis(7)));
        frame.dirty_at = None;
        assert_eq!(frame.work_start(), frame.signal_at);

        frame.refresh_interval = Some(Duration::from_millis(3));
        assert_eq!(frame.budget(), FRAME_BUDGET_FLOOR);
        let fast_frame_end = start + Duration::from_millis(11);
        assert_eq!(frame.work(fast_frame_end), Some(Duration::from_millis(6)));
        assert_eq!(frame.is_late(fast_frame_end), Some(false));
        assert_eq!(frame.missed_refreshes(fast_frame_end), Some(1));
        frame.refresh_interval = None;
        assert_eq!(frame.budget(), UNKNOWN_REFRESH_FRAME_BUDGET);
        assert_eq!(frame.work(fast_frame_end), Some(Duration::from_millis(6)));
        assert_eq!(frame.missed_refreshes(fast_frame_end), None);
        frame.signal_at = None;
        assert_eq!(frame.work_start(), None);
        assert_eq!(frame.work(submitted_at), None);
        assert_eq!(frame.is_late(submitted_at), None);
        assert_eq!(frame.missed_refreshes(submitted_at), None);
    }

    #[test]
    fn downstream_owned_draw_reports_keep_refresh_rows_and_snapshot_deltas() {
        let _restore = RestoreTrace(profiler::trace_enabled());
        profiler::set_trace_enabled(false);
        let mut owner = WindowProfiler::new(WindowId::from(9)).unwrap();
        let sixty = Some(Duration::from_secs(1) / 60);
        for interval in [sixty, Some(Duration::from_secs(1) / 240), None] {
            owner.set_refresh_interval(interval);
            owner.record_frame_signal(None);
            owner.begin_draw();
            owner.record_draw_phases(DrawPhases::default());
            owner.end_draw(None, 1);
        }
        let before = owner.frame_duration_snapshot();
        assert_eq!(before.draw_duration_histogram.len(), 3);
        assert_eq!(before.dirty_to_present_histogram.len(), 0);
        assert_eq!(before.present_interval_histogram.len(), 0);
        assert_eq!(before.by_refresh_interval.len(), 3);
        for frames in &before.by_refresh_interval {
            assert_eq!(frames.frames_drawn, 1);
            assert_eq!(frames.draw_duration.len(), 1);
            assert_eq!(frames.frames_on_time, 0);
            assert_eq!(frames.frames_late, 0);
            assert_eq!(frames.frames_unmeasured, 0);
            assert_eq!(frames.missed_refreshes, 0);
            assert_eq!(frames.work.len(), 0);
            assert_eq!(frames.input_frames_on_time, 0);
            assert_eq!(frames.input_frames_late, 0);
            assert_eq!(frames.input_work.len(), 0);
        }
        owner.set_refresh_interval(sixty);
        owner.begin_draw();
        owner.end_draw(None, 1);
        let after = owner.frame_duration_snapshot();
        let earlier = before
            .by_refresh_interval
            .iter()
            .find(|frames| frames.refresh_interval == sixty)
            .unwrap();
        let later = after
            .by_refresh_interval
            .iter()
            .find(|frames| frames.refresh_interval == sixty)
            .unwrap();
        let delta = later.since(earlier);
        assert_eq!(delta.refresh_interval, sixty);
        assert_eq!(delta.frames_drawn, 1);
        assert_eq!(delta.draw_duration.len(), 1);
        assert_eq!(
            delta.frames_on_time + delta.frames_late + delta.frames_unmeasured,
            0
        );
        assert_eq!(delta.missed_refreshes, 0);
        assert_eq!(delta.work.len() + delta.input_work.len(), 0);
        assert_eq!(delta.input_frames_on_time + delta.input_frames_late, 0);
        assert_eq!(before.draw_duration_histogram.len(), 3);
        assert_eq!(earlier.frames_drawn, 1);
        let unchanged = later.since(later);
        assert_eq!(
            unchanged.frames_drawn
                + unchanged.frames_on_time
                + unchanged.frames_late
                + unchanged.frames_unmeasured
                + unchanged.missed_refreshes,
            0
        );
        assert_eq!(
            unchanged.draw_duration.len() + unchanged.work.len() + unchanged.input_work.len(),
            0
        );
        assert_eq!(
            unchanged.input_frames_on_time + unchanged.input_frames_late,
            0
        );
    }

    #[test]
    fn downstream_histogram_buckets_include_boundaries_and_a_final_overflow_bucket() {
        let owner = WindowProfiler::new(WindowId::from(11)).unwrap();
        let mut histogram = owner.frame_duration_snapshot().draw_duration_histogram;
        for millis in [1, 4, 5, 8, 9, 34, 1100] {
            histogram.record(millis * 1_000_000).unwrap();
        }
        assert_eq!(
            histogram_bucket_counts(
                &histogram,
                FRAME_DURATION_BUCKETS_MS.map(|ms| ms * 1_000_000)
            ),
            [2, 2, 1, 0, 1, 0, 0, 0, 0, 1],
        );
        histogram.reset();
        for percent in [25, 50, 75, 100, 150, 200, 400, 401] {
            histogram.record(percent).unwrap();
        }
        assert_eq!(
            histogram_bucket_counts(&histogram, FRAME_WORK_BUCKETS_PERCENT),
            [1, 1, 1, 1, 1, 1, 1, 1],
        );
        assert_eq!(histogram_bucket_counts(&histogram, []), [8]);
    }

    #[test]
    fn downstream_view_values_serialize_v3_names_phases_and_exclusive_work() {
        let start = RunnableMeta::new_with_callers_location().spawned.0;
        let parent = ForegroundEvent::View(ViewTiming {
            name: "consumer::Parent",
            phase: ViewPhase::RequestLayout,
            start: start + Duration::from_millis(1),
            end: start + Duration::from_millis(5),
            self_duration: Duration::from_millis(2),
        });
        let child = ForegroundEvent::View(ViewTiming {
            name: "consumer::Child",
            phase: ViewPhase::Paint,
            start: start + Duration::from_millis(2),
            end: start + Duration::from_millis(4),
            self_duration: Duration::from_millis(2),
        });
        assert_eq!(parent.duration(), Duration::from_millis(4));
        let mut sealer = IntervalSealer::new(start);
        let snapshots = sealer.push_entries([
            ForegroundJournalEntry::Event(child),
            ForegroundJournalEntry::Event(parent),
            ForegroundJournalEntry::Boundary(IntervalBoundary::Idle {
                ended_at: start + Duration::from_millis(6),
            }),
        ]);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].occupancy(), Duration::from_millis(4));
        let incident = HangIncident::detect(
            snapshots.into_iter().next().unwrap(),
            Duration::from_millis(2),
            Duration::MAX,
        )
        .unwrap();
        let serialized = SerializedHangIncident::convert(start, &incident, usize::MAX, None);
        assert_eq!(MEASUREMENT_VERSION, 3);
        assert_eq!(serialized.measurement_version, 3);
        assert_eq!(serialized.trigger, HangTrigger::Threshold);
        assert!(
            !serialized.during_active_use,
            "only a detector can establish active use"
        );
        assert_eq!(serialized.sealed_by, "idle");
        assert_eq!(serialized.event_count, 2);
        assert_eq!(serialized.contributors_elided, 0);
        assert!(matches!(
            serialized.contributors.as_slice(),
            [
                SerializedHangContributor::View {
                    name: "consumer::Parent",
                    phase: "request_layout",
                    duration_ms: parent_duration,
                    self_ms: parent_self,
                    depth: 0,
                    ..
                },
                SerializedHangContributor::View {
                    name: "consumer::Child",
                    phase: "paint",
                    duration_ms: child_duration,
                    self_ms: child_self,
                    depth: 1,
                    ..
                },
            ] if *parent_duration == 4.0 && *parent_self == 2.0
                && *child_duration == 2.0 && *child_self == 2.0
        ));
        assert_eq!(ViewPhase::Prepaint.as_str(), "prepaint");
        assert_eq!(
            HangTrigger::ALL.map(HangTrigger::as_str),
            ["threshold", "late_frame", "blocking_work"]
        );
    }

    #[test]
    fn downstream_input_activity_controls_do_not_create_work_or_sealed_intervals() {
        use open_gpui::profiler::journal::InputActivity;

        let at = RunnableMeta::new_with_callers_location().spawned.0;
        let mut sealer = IntervalSealer::new(at);
        let raw = [
            ForegroundJournalEntry::InputActivity(InputActivity { at, active: true }),
            ForegroundJournalEntry::InputActivity(InputActivity {
                at: at + Duration::from_millis(1),
                active: false,
            }),
        ];
        assert!(sealer.push_entries(raw).is_empty());
        assert!(
            sealer
                .push_entries([ForegroundJournalEntry::Boundary(IntervalBoundary::Idle {
                    ended_at: at + Duration::from_millis(2),
                }),])
                .is_empty()
        );
    }
}

#[cfg(not(feature = "profiler"))]
#[test]
fn downstream_disabled_timing_scopes_and_collectors_stay_empty() {
    use open_gpui::{ProfilingCollector, RunnableMeta, TasksIncluded, profiler};

    assert!(!profiler::trace_enabled());
    assert!(!profiler::set_trace_enabled(true));
    let metadata = RunnableMeta::new_with_callers_location();
    let mut collector = ProfilingCollector::new(metadata.spawned.0);
    {
        let _outer = profiler::task_timing_unqueued(metadata.spawned, metadata.location);
        let _inner = profiler::task_timing(metadata.spawned, metadata.location);
        let running = profiler::get_current_thread_timings(TasksIncluded::CompletedAndRunning);
        assert!(running.timings.is_empty());
        assert_eq!(running.total_pushed, 0);
        assert!(collector.collect_unseen(vec![running]).is_empty());
    }
    for include_running in [false, true] {
        let included = || {
            if include_running {
                TasksIncluded::CompletedAndRunning
            } else {
                TasksIncluded::OnlyCompleted
            }
        };
        assert!(profiler::get_all_timings(included()).is_empty());
        assert!(profiler::take_all_stats(included()).is_empty());
        let completed = profiler::get_current_thread_timings(included());
        assert!(completed.timings.is_empty());
        assert_eq!(completed.total_pushed, 0);
        assert!(collector.collect_unseen(vec![completed]).is_empty());
    }
    collector.reset();
    assert_eq!(collector.startup_time(), metadata.spawned.0);
    assert!(!profiler::set_trace_enabled(false));
    assert!(!profiler::trace_enabled());
    assert!(profiler::take_action_stats().is_empty());
}
