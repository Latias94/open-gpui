//! Ordinary profiler consumers without test-support or an App/Window instance.
//! Window closures provide API evidence only. Executed controls cover trace flags,
//! task snapshots and collectors, not input-to-Submitted timing or native behavior.

use open_gpui::{RunnableMeta, TasksIncluded, profiler};

struct RestoreTraceFlag(bool);

impl Drop for RestoreTraceFlag {
    fn drop(&mut self) {
        profiler::set_trace_enabled(self.0);
    }
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_window_profiling_signatures_do_not_require_test_support() {
    use open_gpui::{
        FrameDurationSnapshot, FrameEvent, FrameTiming, InputLatencySnapshot, PresentTiming,
        Window, WindowProfiler,
    };
    use std::time::Duration;

    let _: fn(&Window) -> InputLatencySnapshot = Window::input_latency_snapshot;
    let _: fn(&Window) -> FrameDurationSnapshot = Window::frame_duration_snapshot;
    let _: fn(&mut Window) = Window::reset_input_latency_measurements;
    let _: fn(&FrameTiming) -> Duration = FrameTiming::draw_duration;
    let _: fn(&PresentTiming) -> Duration = PresentTiming::present_duration;

    let _window_consumer = |window: &mut Window| {
        let input: InputLatencySnapshot = window.input_latency_snapshot();
        let frame: FrameDurationSnapshot = window.frame_duration_snapshot();
        let counts: [u64; 9] = [
            input.pending_input_count,
            input.drawn_frame_count,
            input.present_attempt_count,
            input.submitted_present_count,
            input.deferred_present_count,
            input.repaint_required_present_count,
            input.rejected_present_count,
            input.refresh_request_count,
            input.coalesced_refresh_request_count,
        ];
        window.reset_input_latency_measurements();
        (input, frame, counts)
    };

    let _owned_profiler_consumer = |window: &Window, presentation: PresentTiming| {
        let mut owner = WindowProfiler::new(window.window_handle().window_id())
            .expect("valid profiler histograms");
        owner.begin_input("downstream-input");
        owner.end_input(true);
        owner.begin_draw();
        owner.end_draw(None, 1);
        owner.record_present(
            presentation.present_start,
            presentation.present_end,
            true,
            false,
        );
        let input: InputLatencySnapshot = owner.input_latency_snapshot();
        let frame: FrameDurationSnapshot = owner.frame_duration_snapshot();
        (owner, input, frame)
    };

    let _event_consumer = |event: FrameEvent| match event {
        FrameEvent::Draw(timing) => (timing.window_id, timing.draw_duration()),
        FrameEvent::Present(timing) => (timing.window_id, timing.present_duration()),
    };
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_trace_controls_collect_and_clear_task_snapshots_without_an_app() {
    use open_gpui::{FrameEvent, FrameTimingCollector, ProfilingCollector};

    let _restore = RestoreTraceFlag(profiler::trace_enabled());
    profiler::set_trace_enabled(false);
    assert!(!profiler::trace_enabled());
    assert!(!profiler::set_trace_enabled(false));
    let initial_total =
        profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted).total_pushed;

    let metadata = RunnableMeta::new_with_callers_location();
    let mut tasks = ProfilingCollector::new(metadata.spawned.0);
    let mut frames = FrameTimingCollector::new();
    assert!(profiler::set_trace_enabled(true));
    assert!(profiler::trace_enabled());
    assert!(!profiler::set_trace_enabled(true));

    profiler::update_running_task(metadata.spawned, metadata.location);
    profiler::save_task_timing();

    let snapshot = profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted);
    assert_eq!(snapshot.timings.len(), 1);
    assert_eq!(snapshot.total_pushed, initial_total + 1);
    let total_before_clear = snapshot.total_pushed;
    let delta = tasks.collect_unseen(vec![snapshot]);
    assert_eq!(delta.len(), 1);
    assert_eq!(delta[0].new_timings.len(), 1);
    let unchanged = profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted);
    assert!(tasks.collect_unseen(vec![unchanged]).is_empty());
    let frame_events: Vec<FrameEvent> = frames.collect_unseen();
    assert!(frame_events.is_empty());

    assert!(profiler::set_trace_enabled(false));
    assert!(!profiler::trace_enabled());
    let cleared = profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted);
    assert!(cleared.timings.is_empty());
    assert!(cleared.total_pushed >= total_before_clear);
    let cleared_total = cleared.total_pushed;
    assert!(tasks.collect_unseen(vec![cleared]).is_empty());
    assert!(frames.collect_unseen().is_empty());

    assert!(profiler::set_trace_enabled(true));
    let metadata = RunnableMeta::new_with_callers_location();
    profiler::update_running_task(metadata.spawned, metadata.location);
    profiler::save_task_timing();
    let resumed = profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted);
    assert_eq!(resumed.timings.len(), 1);
    assert!(resumed.total_pushed > cleared_total);
    let delta = tasks.collect_unseen(vec![resumed]);
    assert_eq!(delta.len(), 1);
    assert_eq!(delta[0].new_timings.len(), 1);
    let unchanged = profiler::get_current_thread_timings(TasksIncluded::OnlyCompleted);
    assert!(tasks.collect_unseen(vec![unchanged]).is_empty());
    assert!(frames.collect_unseen().is_empty());
}

#[cfg(not(feature = "profiler"))]
#[test]
fn downstream_disabled_profiler_aggregate_accessors_remain_empty() {
    let _restore = RestoreTraceFlag(profiler::trace_enabled());
    profiler::set_trace_enabled(true);
    let metadata = RunnableMeta::new_with_callers_location();
    profiler::update_running_task(metadata.spawned, metadata.location);
    profiler::save_task_timing();

    assert!(profiler::get_all_timings(TasksIncluded::OnlyCompleted).is_empty());
    assert!(profiler::get_all_timings(TasksIncluded::CompletedAndRunning).is_empty());
    assert!(profiler::take_all_stats(TasksIncluded::CompletedAndRunning).is_empty());
    let snapshot = profiler::get_current_thread_timings(TasksIncluded::CompletedAndRunning);
    assert!(snapshot.timings.is_empty());
    assert_eq!(snapshot.total_pushed, 0);
    assert!(
        snapshot
            .stats
            .longest_poll_times
            .iter()
            .chain(&snapshot.stats.longest_runtimes)
            .all(|timing| timing.poll_duration().is_zero())
    );
    let actions = profiler::take_action_stats();
    assert!(actions.is_empty());
    assert!(actions.longest_runtimes(true).next().is_none());
}
