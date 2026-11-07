#![cfg(all(feature = "test-support", feature = "profiler"))]

use open_gpui::{
    App, AppContext as _, Context, Entity, FrameDurationSnapshot, FrameRequestSource,
    FrameTimingCollector, InputLatencySnapshot, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, PlatformInput, PlatformWindowPresentOutcome, Render,
    RequestFrameOptions, Styled as _, TestAppContext, ViewPhase, VisualTestContext, Window,
    WindowHandle, WindowInitialPresentationStatus, WindowPresentationFacts, canvas, div, point,
    profiler::{
        self, FRAME_BUDGET_FLOOR, FRAME_DURATION_BUCKETS_MS, FRAME_WORK_BUCKETS_PERCENT,
        RefreshIntervalFrames,
        hang::{HangDetector, HangTrigger, MEASUREMENT_VERSION, SerializedHangIncident},
        histogram_bucket_counts,
        journal::{ForegroundEvent, ForegroundJournalEntry, IntervalBoundary, PresentedFrame},
    },
    px, size,
};
use open_gpui_scheduler::Instant;
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Default)]
struct Observations {
    inputs: Cell<u64>,
    renders: Cell<u64>,
    input_delay: Cell<Duration>,
    parent_delay: Cell<Duration>,
    child_delay: Cell<Duration>,
    prepaint_delay: Cell<Duration>,
    paint_delay: Cell<Duration>,
}

struct MeasuredChild(Rc<Observations>);

impl Render for MeasuredChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if !self.0.child_delay.get().is_zero() {
            std::thread::sleep(self.0.child_delay.get());
        }
        div().size(px(24.)).bg(open_gpui::green())
    }
}

struct MeasuredView {
    observed: Rc<Observations>,
    child: Entity<MeasuredChild>,
}

impl Render for MeasuredView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        if !self.observed.parent_delay.get().is_zero() {
            std::thread::sleep(self.observed.parent_delay.get());
        }
        let observed = Rc::clone(&self.observed);
        let prepaint = Rc::clone(&observed);
        let paint = Rc::clone(&observed);
        div()
            .size_full()
            .bg(if observed.inputs.get().is_multiple_of(2) {
                open_gpui::red()
            } else {
                open_gpui::blue()
            })
            .child(self.child.clone())
            .child(
                canvas(
                    move |_, _, _| {
                        if !prepaint.prepaint_delay.get().is_zero() {
                            std::thread::sleep(prepaint.prepaint_delay.get());
                        }
                    },
                    move |_, (), _, _| {
                        if !paint.paint_delay.get().is_zero() {
                            std::thread::sleep(paint.paint_delay.get());
                        }
                    },
                )
                .size(px(8.)),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                observed.inputs.set(observed.inputs.get() + 1);
                if !observed.input_delay.get().is_zero() {
                    std::thread::sleep(observed.input_delay.get());
                }
                window.refresh();
            })
    }
}

struct RestoreTrace(bool);

impl Drop for RestoreTrace {
    fn drop(&mut self) {
        profiler::set_trace_enabled(self.0);
    }
}

fn facts(visual: &mut VisualTestContext) -> WindowPresentationFacts {
    visual.update(|window, _| window.presentation_facts())
}

fn assert_submitted(facts: WindowPresentationFacts, generation: u64) {
    assert!(facts.native_created);
    assert!(facts.native_visible);
    assert_eq!(
        facts.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert_eq!(facts.frame_accepted_generation, Some(generation));
    assert_eq!(facts.present_submitted_generation, Some(generation));
    assert_eq!(facts.non_empty_presented_generation, Some(generation));
    let attempt = facts.latest_present_attempt.unwrap();
    assert_eq!(attempt.generation, generation);
    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
    assert!(attempt.contained_valid_primitives);
}

fn open_parked_window(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<MeasuredView>,
    VisualTestContext,
    Rc<Observations>,
) {
    let observed = Rc::new(Observations::default());
    cx.defer_next_window_frame_requests();
    let handle = cx.open_window(size(px(96.), px(96.)), {
        let observed = Rc::clone(&observed);
        move |_, cx| MeasuredView {
            child: cx.new(|_| MeasuredChild(Rc::clone(&observed))),
            observed,
        }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, app| {
        assert!(window.is_window_active());
        window.draw(app).clear(app);
    });
    assert!(visual.frame_scheduled());
    assert!(visual.simulate_scheduled_frame());
    let submitted = facts(&mut visual);
    assert_submitted(submitted, submitted.frame_accepted_generation.unwrap());
    assert!(visual.simulate_scheduled_frame());
    assert!(!visual.frame_scheduled());
    assert!(!visual.simulate_scheduled_frame());
    assert_eq!(facts(&mut visual), submitted);
    (handle, visual, observed)
}

fn dispatch_input(window: &mut Window, app: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(40.), px(40.)),
            button: MouseButton::Left,
            ..Default::default()
        }),
        app,
    );
}

fn queue_input(visual: &mut VisualTestContext) {
    visual.update(|window, _| window.on_next_frame(dispatch_input));
    assert!(visual.frame_scheduled());
}

fn measured_callback(visual: &mut VisualTestContext) -> Instant {
    let signal_at = Instant::now();
    assert!(visual.simulate_frame(RequestFrameOptions {
        signal_at: Some(signal_at),
        signal_source: FrameRequestSource::LocalSchedule,
        require_presentation: true,
        ..Default::default()
    }));
    signal_at
}

fn snapshots(visual: &mut VisualTestContext) -> (InputLatencySnapshot, FrameDurationSnapshot) {
    visual.update(|window, _| {
        (
            window.input_latency_snapshot(),
            window.frame_duration_snapshot(),
        )
    })
}

fn refresh_delta(
    before: &FrameDurationSnapshot,
    after: &FrameDurationSnapshot,
    interval: Option<Duration>,
) -> RefreshIntervalFrames {
    let current = after
        .by_refresh_interval
        .iter()
        .find(|frames| frames.refresh_interval == interval)
        .expect("the accepted draw must have a refresh-interval row");
    before
        .by_refresh_interval
        .iter()
        .find(|frames| frames.refresh_interval == interval)
        .map_or_else(|| current.clone(), |earlier| current.since(earlier))
}

fn presented(
    entries: &[ForegroundJournalEntry],
    handle: WindowHandle<MeasuredView>,
) -> PresentedFrame {
    let frames = entries
        .iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented))
                if presented.frame.window_id == handle.window_id() =>
            {
                Some(*presented)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        frames.len(),
        1,
        "one newly accepted scene must seal exactly once"
    );
    frames[0]
}

fn assert_one_draw_and_input(before: &InputLatencySnapshot, after: &InputLatencySnapshot) {
    assert_eq!(after.drawn_frame_count, before.drawn_frame_count + 1);
    assert_eq!(
        after.present_attempt_count,
        before.present_attempt_count + 1
    );
    assert_eq!(
        after.submitted_present_count,
        before.submitted_present_count + 1
    );
    assert_eq!(after.deferred_present_count, before.deferred_present_count);
    assert_eq!(
        after.repaint_required_present_count,
        before.repaint_required_present_count
    );
    assert_eq!(after.rejected_present_count, before.rejected_present_count);
    assert_eq!(after.pending_input_count, 0);
    assert_eq!(
        after.mid_draw_events_dropped,
        before.mid_draw_events_dropped
    );
    assert_eq!(
        after.latency_histogram.len(),
        before.latency_histogram.len() + 1
    );
    assert_eq!(
        after.events_per_frame_histogram.len(),
        before.events_per_frame_histogram.len() + 1
    );
    assert_eq!(
        after.refresh_request_count,
        before.refresh_request_count + 1
    );
    assert_eq!(
        after.coalesced_refresh_request_count,
        before.coalesced_refresh_request_count
    );
    assert_eq!(
        after
            .refresh_callsites
            .iter()
            .map(|site| site.request_count)
            .sum::<u64>(),
        after.refresh_request_count
    );
    assert_eq!(
        after
            .refresh_callsites
            .iter()
            .map(|site| site.coalesced_request_count)
            .sum::<u64>(),
        after.coalesced_refresh_request_count
    );
}

#[open_gpui::test]
fn public_callback_to_report_tracks_changed_and_unknown_display_refresh(cx: &mut TestAppContext) {
    let _restore = RestoreTrace(profiler::trace_enabled());
    profiler::set_trace_enabled(false);
    let (handle, mut visual, observed) = open_parked_window(cx);
    let display_id = visual.update(|window, app| window.display(app).unwrap().id());
    let mut trace = FrameTimingCollector::new();

    for interval in [
        Some(Duration::from_secs(1) / 60),
        Some(Duration::from_secs(1) / 240),
        None,
        Some(Duration::from_secs(1) / 30),
    ] {
        cx.simulate_display_refresh_interval_change(display_id, interval);
        visual.update(|window, app| assert_eq!(window.refresh_interval(app), interval));
        let baseline = facts(&mut visual);
        let (before_input, before_report) = snapshots(&mut visual);
        let start = Instant::now();
        let journal = cx.read(|app| app.foreground_journal());
        let mut collector = journal.collector();
        let mut detector = HangDetector::new(journal, Duration::ZERO, Duration::MAX);
        assert_eq!(
            detector.take_active_time(start, Duration::MAX),
            Duration::ZERO
        );
        let renders = observed.renders.get();
        let inputs = observed.inputs.get();
        queue_input(&mut visual);
        assert_eq!(observed.renders.get(), renders);
        let signal_at = measured_callback(&mut visual);
        assert_eq!(observed.inputs.get(), inputs + 1);
        assert_eq!(observed.renders.get(), renders + 1);
        assert_submitted(
            facts(&mut visual),
            baseline.frame_accepted_generation.unwrap() + 1,
        );

        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        let submitted = presented(&drained.entries, handle);
        let frame = submitted.frame;
        assert_eq!(frame.refresh_interval, interval);
        assert_eq!(frame.signal_at, Some(signal_at));
        assert!(drained.entries.iter().any(|entry| matches!(
            entry, ForegroundJournalEntry::PlatformSignal(signal)
                if signal.window_id == handle.window_id()
                    && signal.signal_at == Some(signal_at)
                    && signal.source == FrameRequestSource::LocalSchedule
        )));
        assert!(frame.invalidations > 0);
        assert!(frame.dirty_at.unwrap() >= signal_at);
        assert_eq!(frame.work_start(), frame.dirty_at);
        assert!(frame.draw_start >= frame.dirty_at.unwrap());
        assert!(frame.draw_end <= submitted.presentation.present_start);
        assert_eq!(
            frame.work(submitted.presentation.present_start),
            Some(
                submitted
                    .presentation
                    .present_start
                    .duration_since(frame.work_start().unwrap())
            )
        );
        assert!(
            frame.phases.request_layout + frame.phases.prepaint + frame.phases.paint
                <= frame.draw_duration()
        );

        let (after_input, after_report) = snapshots(&mut visual);
        assert_one_draw_and_input(&before_input, &after_input);
        assert_eq!(
            after_report.draw_duration_histogram.len(),
            before_report.draw_duration_histogram.len() + 1
        );
        assert_eq!(
            after_report.dirty_to_present_histogram.len(),
            before_report.dirty_to_present_histogram.len() + 1
        );
        assert_eq!(
            after_report.present_interval_histogram.len(),
            before_report.present_interval_histogram.len()
        );
        let delta = refresh_delta(&before_report, &after_report, interval);
        assert_eq!(delta.frames_drawn, 1);
        assert_eq!(delta.draw_duration.len(), 1);
        assert_eq!(delta.frames_on_time + delta.frames_late, 1);
        assert_eq!(
            delta.frames_late,
            u64::from(frame.is_late(submitted.presentation.present_start).unwrap())
        );
        assert_eq!(delta.frames_unmeasured, 0);
        assert_eq!(
            delta.missed_refreshes,
            frame
                .missed_refreshes(submitted.presentation.present_start)
                .unwrap_or(0)
        );
        assert_eq!(delta.work.len(), 1);
        assert_eq!(delta.input_frames_on_time, delta.frames_on_time);
        assert_eq!(delta.input_frames_late, delta.frames_late);
        assert_eq!(delta.input_work.len(), 1);
        let draw_buckets = histogram_bucket_counts(
            &delta.draw_duration,
            FRAME_DURATION_BUCKETS_MS.map(|ms| ms * 1_000_000),
        );
        assert_eq!(draw_buckets.len(), FRAME_DURATION_BUCKETS_MS.len() + 1);
        assert_eq!(draw_buckets.iter().sum::<u64>(), 1);
        if let Some(interval) = interval {
            let bounds = FRAME_WORK_BUCKETS_PERCENT.map(|percent| {
                u64::try_from((interval * u32::try_from(percent).unwrap() / 100).as_nanos())
                    .unwrap()
            });
            assert_eq!(
                histogram_bucket_counts(&delta.work, bounds)
                    .iter()
                    .sum::<u64>(),
                1
            );
        }

        let incidents = detector.poll();
        let incident = incidents
            .iter()
            .find(|incident| {
                incident.snapshot.interval_end() == submitted.presentation.present_end
                    && matches!(incident.snapshot.boundary, IntervalBoundary::Presented(_))
            })
            .expect("the independent detector must observe the same submitted frame");
        assert_eq!(incident.trigger, HangTrigger::Threshold);
        assert!(incident.during_active_use);
        assert_eq!(
            detector.first_present_at(),
            Some(submitted.presentation.present_end)
        );
        let json = serde_json::to_value(SerializedHangIncident::convert(
            start,
            incident,
            usize::MAX,
            detector.first_present_at(),
        ))
        .unwrap();
        assert_eq!(MEASUREMENT_VERSION, 3);
        assert_eq!(json["measurement_version"], 3);
        assert_eq!(json["during_active_use"], true);
        assert_eq!(json["trigger"], "threshold");
        assert_eq!(json["sealed_by"], "present");
        for kind in ["input", "draw", "present"] {
            assert!(
                json["contributors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["kind"] == kind)
            );
        }
        let now = Instant::now();
        let elapsed = now.duration_since(start);
        let active = detector.take_active_time(now, elapsed);
        assert!(active > Duration::ZERO && active <= elapsed);
        assert!(detector.poll().is_empty());
        assert!(collector.collect_unseen().entries.is_empty());
        assert!(
            trace.collect_unseen().is_empty(),
            "reports and journals must work with tracing disabled"
        );
        assert!(visual.simulate_scheduled_frame());
        assert!(!visual.frame_scheduled());
        assert!(!visual.simulate_scheduled_frame());
    }
}

#[open_gpui::test]
fn public_scheduled_callbacks_without_a_signal_keep_frames_unmeasured(cx: &mut TestAppContext) {
    let (handle, mut visual, observed) = open_parked_window(cx);
    let baseline = facts(&mut visual);
    let (before_input, before_report) = snapshots(&mut visual);
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    queue_input(&mut visual);
    assert!(visual.simulate_scheduled_frame());
    assert_eq!(observed.inputs.get(), 1);
    assert_submitted(
        facts(&mut visual),
        baseline.frame_accepted_generation.unwrap() + 1,
    );
    let drained = collector.collect_unseen();
    assert_eq!(drained.lost, 0);
    let submitted = presented(&drained.entries, handle);
    assert_eq!(submitted.frame.signal_at, None);
    assert_eq!(
        submitted.frame.work(submitted.presentation.present_start),
        None
    );
    assert_eq!(
        submitted
            .frame
            .is_late(submitted.presentation.present_start),
        None
    );
    assert_eq!(
        submitted
            .frame
            .missed_refreshes(submitted.presentation.present_start),
        None
    );
    let (after_input, after_report) = snapshots(&mut visual);
    assert_one_draw_and_input(&before_input, &after_input);
    let delta = refresh_delta(
        &before_report,
        &after_report,
        submitted.frame.refresh_interval,
    );
    assert_eq!(delta.frames_drawn, 1);
    assert_eq!(delta.draw_duration.len(), 1);
    assert_eq!(delta.frames_unmeasured, 1);
    assert_eq!(
        delta.frames_on_time + delta.frames_late + delta.missed_refreshes,
        0
    );
    assert_eq!(delta.work.len() + delta.input_work.len(), 0);
    assert_eq!(delta.input_frames_on_time + delta.input_frames_late, 0);
    assert!(visual.simulate_scheduled_frame());
    assert!(!visual.frame_scheduled());
    let (_, unchanged) = snapshots(&mut visual);
    let idle_delta = refresh_delta(&after_report, &unchanged, submitted.frame.refresh_interval);
    assert_eq!(idle_delta.frames_drawn + idle_delta.frames_unmeasured, 0);
    assert_eq!(
        idle_delta.draw_duration.len() + idle_delta.work.len() + idle_delta.input_work.len(),
        0
    );
}

#[open_gpui::test]
fn public_no_render_callback_cannot_lend_its_signal_to_a_later_direct_draw(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_parked_window(cx);
    let baseline = facts(&mut visual);
    let (_, before_report) = snapshots(&mut visual);
    let renders = observed.renders.get();
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    visual.update(|window, _| window.on_next_frame(|_, _| {}));
    let skipped_signal = Instant::now();
    assert!(visual.simulate_frame(RequestFrameOptions {
        signal_at: Some(skipped_signal),
        signal_source: FrameRequestSource::LocalSchedule,
        ..Default::default()
    }));
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(facts(&mut visual), baseline);
    let skipped = collector.collect_unseen();
    assert_eq!(skipped.lost, 0);
    assert!(skipped.entries.iter().any(|entry| matches!(entry,
        ForegroundJournalEntry::PlatformSignal(signal)
            if signal.window_id == handle.window_id()
                && signal.signal_at == Some(skipped_signal)
                && signal.source == FrameRequestSource::LocalSchedule
    )));
    assert!(
        !skipped
            .entries
            .iter()
            .any(|entry| matches!(entry, ForegroundJournalEntry::Boundary(_)))
    );
    assert!(!skipped.entries.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Event(ForegroundEvent::Draw(_))
            | ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
    )));

    visual.update(|window, app| {
        window.refresh();
        window.draw(app).clear(app);
    });
    assert_eq!(observed.renders.get(), renders + 1);
    let accepted = facts(&mut visual);
    let generation = baseline.frame_accepted_generation.unwrap() + 1;
    assert_eq!(accepted.frame_accepted_generation, Some(generation));
    assert_eq!(
        accepted.present_submitted_generation,
        baseline.present_submitted_generation
    );
    let direct = collector.collect_unseen();
    assert_eq!(direct.lost, 0);
    let draw = direct
        .entries
        .iter()
        .find_map(|entry| match entry {
            ForegroundJournalEntry::Event(ForegroundEvent::Draw(frame))
                if frame.window_id == handle.window_id() =>
            {
                Some(*frame)
            }
            _ => None,
        })
        .expect("the public direct draw must publish its own frame");
    assert_eq!(draw.signal_at, None);
    assert_eq!(draw.work_start(), None);

    assert!(visual.frame_scheduled());
    assert!(visual.simulate_scheduled_frame());
    assert_submitted(facts(&mut visual), generation);
    assert_eq!(observed.renders.get(), renders + 1);
    let completed = collector.collect_unseen();
    assert_eq!(completed.lost, 0);
    let submitted = presented(&completed.entries, handle);
    assert_eq!(submitted.frame.signal_at, None);
    assert_eq!(submitted.frame.draw_start, draw.draw_start);
    assert_eq!(submitted.frame.phases, draw.phases);
    let (_, after_report) = snapshots(&mut visual);
    let delta = refresh_delta(&before_report, &after_report, draw.refresh_interval);
    assert_eq!(delta.frames_drawn, 1);
    assert_eq!(delta.frames_unmeasured, 1);
    assert_eq!(delta.frames_on_time + delta.frames_late, 0);
    assert_eq!(delta.work.len() + delta.input_work.len(), 0);
}

#[open_gpui::test]
fn public_failed_presentations_keep_the_original_measured_scene_until_submission(
    cx: &mut TestAppContext,
) {
    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::Rejected,
    ] {
        let (handle, mut visual, observed) = open_parked_window(cx);
        let display_id = visual.update(|window, app| window.display(app).unwrap().id());
        cx.simulate_display_refresh_interval_change(display_id, Some(Duration::from_secs(1) / 60));
        let baseline = facts(&mut visual);
        let (before_input, before_report) = snapshots(&mut visual);
        let mut collector = cx.read(|app| app.foreground_journal().collector());
        cx.set_window_present_outcome(handle.into(), outcome);
        queue_input(&mut visual);
        let signal_at = measured_callback(&mut visual);
        let accepted = facts(&mut visual);
        let generation = baseline.frame_accepted_generation.unwrap() + 1;
        let renders = observed.renders.get();
        assert_eq!(accepted.frame_accepted_generation, Some(generation));
        assert_eq!(
            accepted.present_submitted_generation,
            baseline.present_submitted_generation
        );
        assert_eq!(
            accepted.non_empty_presented_generation,
            baseline.non_empty_presented_generation
        );
        assert_eq!(accepted.latest_present_attempt.unwrap().outcome, outcome);
        let pending = collector.collect_unseen();
        assert_eq!(pending.lost, 0);
        let drawn = pending
            .entries
            .iter()
            .find_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(frame))
                    if frame.window_id == handle.window_id() =>
                {
                    Some(*frame)
                }
                _ => None,
            })
            .expect("the initial renderer attempt must use an accepted draw");
        let interval = drawn.refresh_interval;
        assert!(!pending.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));

        let changed_interval = Some(Duration::from_secs(1) / 240);
        assert_ne!(interval, changed_interval);
        cx.simulate_display_refresh_interval_change(display_id, changed_interval);
        visual.update(|window, app| assert_eq!(window.refresh_interval(app), changed_interval));
        assert!(visual.simulate_scheduled_frame());
        assert_eq!(facts(&mut visual), accepted);
        assert_eq!(observed.renders.get(), renders);
        let retry = collector.collect_unseen();
        assert_eq!(retry.lost, 0);
        assert!(!retry.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )));
        let (pending_input, pending_report) = snapshots(&mut visual);
        let pending_delta = refresh_delta(&before_report, &pending_report, interval);
        assert_eq!(pending_delta.frames_drawn, 1);
        assert_eq!(pending_delta.draw_duration.len(), 1);
        assert_eq!(
            pending_delta.frames_on_time
                + pending_delta.frames_late
                + pending_delta.frames_unmeasured,
            0
        );
        assert_eq!(pending_delta.work.len() + pending_delta.input_work.len(), 0);
        assert_eq!(pending_input.pending_input_count, 1);
        assert_eq!(
            pending_input.submitted_present_count,
            before_input.submitted_present_count
        );
        assert_eq!(
            pending_input.latency_histogram.len(),
            before_input.latency_histogram.len()
        );

        cx.set_window_present_outcome(handle.into(), PlatformWindowPresentOutcome::Submitted);
        assert!(visual.simulate_scheduled_frame());
        assert_submitted(facts(&mut visual), generation);
        assert_eq!(observed.renders.get(), renders);
        let final_entries = collector.collect_unseen();
        assert_eq!(final_entries.lost, 0);
        let submitted = presented(&final_entries.entries, handle);
        assert_eq!(submitted.frame.signal_at, Some(signal_at));
        assert_eq!(submitted.frame.refresh_interval, drawn.refresh_interval);
        assert_eq!(submitted.frame.draw_start, drawn.draw_start);
        assert_eq!(submitted.frame.draw_end, drawn.draw_end);
        assert_eq!(submitted.frame.dirty_at, drawn.dirty_at);
        assert_eq!(submitted.frame.invalidations, drawn.invalidations);
        assert_eq!(submitted.frame.phases, drawn.phases);
        let (after_input, after_report) = snapshots(&mut visual);
        assert_eq!(
            after_input.drawn_frame_count,
            before_input.drawn_frame_count + 1
        );
        assert_eq!(
            after_input.present_attempt_count,
            before_input.present_attempt_count + 3
        );
        assert_eq!(
            after_input.submitted_present_count,
            before_input.submitted_present_count + 1
        );
        assert_eq!(
            after_input.deferred_present_count,
            before_input.deferred_present_count
                + if outcome == PlatformWindowPresentOutcome::Deferred {
                    2
                } else {
                    0
                }
        );
        assert_eq!(
            after_input.rejected_present_count,
            before_input.rejected_present_count
                + if outcome == PlatformWindowPresentOutcome::Rejected {
                    2
                } else {
                    0
                }
        );
        assert_eq!(
            after_input.repaint_required_present_count,
            before_input.repaint_required_present_count
        );
        assert_eq!(after_input.pending_input_count, 0);
        assert_eq!(
            after_input.latency_histogram.len(),
            before_input.latency_histogram.len() + 1
        );
        let delta = refresh_delta(&before_report, &after_report, interval);
        assert_eq!(delta.frames_drawn, 1);
        assert_eq!(delta.frames_on_time + delta.frames_late, 1);
        assert_eq!(delta.frames_unmeasured, 0);
        assert_eq!(delta.work.len(), 1);
        assert_eq!(delta.input_work.len(), 1);
        assert_eq!(delta.input_frames_on_time + delta.input_frames_late, 1);
        let changed_row = |report: &FrameDurationSnapshot| {
            report
                .by_refresh_interval
                .iter()
                .find(|frames| frames.refresh_interval == changed_interval)
                .map(|frames| {
                    (
                        frames.frames_drawn,
                        frames.frames_on_time,
                        frames.frames_late,
                        frames.frames_unmeasured,
                        frames.missed_refreshes,
                        frames.draw_duration.len(),
                        frames.work.len(),
                        frames.input_frames_on_time,
                        frames.input_frames_late,
                        frames.input_work.len(),
                    )
                })
        };
        assert_eq!(
            changed_row(&after_report),
            changed_row(&before_report),
            "a retry must not reattribute the old accepted draw to the new display rate"
        );
    }
}

#[open_gpui::test]
fn public_nested_view_phases_reach_v3_incidents_with_exclusive_self_time(cx: &mut TestAppContext) {
    let (handle, mut visual, observed) = open_parked_window(cx);
    observed.parent_delay.set(Duration::from_millis(2));
    observed.child_delay.set(Duration::from_millis(4));
    observed.prepaint_delay.set(Duration::from_millis(2));
    observed.paint_delay.set(Duration::from_millis(2));
    let start = Instant::now();
    let journal = cx.read(|app| app.foreground_journal());
    let mut collector = journal.collector();
    let mut detector = HangDetector::new(journal, Duration::ZERO, Duration::MAX);
    queue_input(&mut visual);
    measured_callback(&mut visual);
    let drained = collector.collect_unseen();
    assert_eq!(drained.lost, 0);
    let submitted = presented(&drained.entries, handle);
    let view = |name| {
        drained
            .entries
            .iter()
            .find_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::View(view))
                    if view.name == name && view.phase == ViewPhase::RequestLayout =>
                {
                    Some(*view)
                }
                _ => None,
            })
            .expect("the slow view must be retained under its public Rust type name")
    };
    let parent = view(std::any::type_name::<MeasuredView>());
    let child = view(std::any::type_name::<MeasuredChild>());
    assert!(parent.start >= submitted.frame.draw_start);
    assert!(parent.end <= submitted.frame.draw_end);
    assert!(child.start >= parent.start && child.end <= parent.end);
    assert!(parent.self_duration >= Duration::from_millis(2));
    assert!(child.self_duration >= Duration::from_millis(4));
    assert!(
        parent.end.duration_since(parent.start) - parent.self_duration
            >= child.end.duration_since(child.start)
    );
    assert!(submitted.frame.phases.request_layout >= parent.end.duration_since(parent.start));
    for phase in [ViewPhase::Prepaint, ViewPhase::Paint] {
        let timing = drained
            .entries
            .iter()
            .find_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::View(view))
                    if view.name == parent.name && view.phase == phase =>
                {
                    Some(*view)
                }
                _ => None,
            })
            .expect("the slow canvas phase must retain its owning view's type name");
        assert!(timing.self_duration >= Duration::from_millis(2));
        assert!(timing.start >= submitted.frame.draw_start);
        assert!(timing.end <= submitted.frame.draw_end);
    }
    let incidents = detector.poll();
    let incident = incidents
        .iter()
        .find(|incident| incident.snapshot.interval_end() == submitted.presentation.present_end)
        .unwrap();
    let json = serde_json::to_value(SerializedHangIncident::convert(
        start,
        incident,
        usize::MAX,
        detector.first_present_at(),
    ))
    .unwrap();
    assert_eq!(json["measurement_version"], 3);
    assert_eq!(json["during_active_use"], true);
    let contributors = json["contributors"].as_array().unwrap();
    for (name, phase) in [
        (parent.name, "request_layout"),
        (child.name, "request_layout"),
        (parent.name, "prepaint"),
        (parent.name, "paint"),
    ] {
        let view = contributors
            .iter()
            .find(|entry| {
                entry["kind"] == "view" && entry["name"] == name && entry["phase"] == phase
            })
            .unwrap();
        assert!(view["self_ms"].as_f64().unwrap() > 0.0);
        assert!(view["duration_ms"].as_f64().unwrap() >= view["self_ms"].as_f64().unwrap());
    }
    let draw = contributors
        .iter()
        .find(|entry| entry["kind"] == "draw")
        .unwrap();
    for phase in ["request_layout_ms", "prepaint_ms", "paint_ms"] {
        assert!(draw[phase].as_f64().unwrap() >= 2.0);
    }
}

#[open_gpui::test]
fn public_measured_late_frame_and_unmeasured_blocking_work_have_distinct_triggers(
    cx: &mut TestAppContext,
) {
    for measured in [true, false] {
        let (handle, mut visual, observed) = open_parked_window(cx);
        let display_id = visual.update(|window, app| window.display(app).unwrap().id());
        cx.simulate_display_refresh_interval_change(display_id, Some(Duration::from_secs(1) / 240));
        if measured {
            observed
                .parent_delay
                .set(FRAME_BUDGET_FLOOR + Duration::from_millis(2));
        } else {
            observed.input_delay.set(Duration::from_millis(2));
        }
        let baseline = facts(&mut visual);
        let start = Instant::now();
        let journal = cx.read(|app| app.foreground_journal());
        let mut collector = journal.collector();
        let mut detector = HangDetector::new(journal, Duration::MAX, Duration::from_millis(1));
        queue_input(&mut visual);
        if measured {
            measured_callback(&mut visual);
        } else {
            assert!(visual.simulate_scheduled_frame());
        }
        assert_submitted(
            facts(&mut visual),
            baseline.frame_accepted_generation.unwrap() + 1,
        );
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        let submitted = presented(&drained.entries, handle);
        let incidents = detector.poll();
        let incident = incidents
            .iter()
            .find(|incident| incident.snapshot.interval_end() == submitted.presentation.present_end)
            .unwrap();
        let expected = if measured {
            HangTrigger::LateFrame
        } else {
            HangTrigger::BlockingWork
        };
        assert_eq!(incident.trigger, expected);
        assert!(incident.during_active_use);
        if measured {
            assert_eq!(submitted.frame.budget(), FRAME_BUDGET_FLOOR);
            assert_eq!(
                submitted
                    .frame
                    .is_late(submitted.presentation.present_start),
                Some(true)
            );
            assert!(
                incident
                    .contributors
                    .iter()
                    .any(|event| matches!(event, ForegroundEvent::View(_)))
            );
            assert!(incident.contributors.iter().all(|event| {
                event.end_time() > submitted.frame.work_start().unwrap()
                    && event.start_time() < submitted.presentation.present_start
            }));
            assert!(
                !incident
                    .contributors
                    .iter()
                    .any(|event| matches!(event, ForegroundEvent::Present(_)))
            );
        } else {
            assert_eq!(submitted.frame.work_start(), None);
            assert!(incident.contributors.iter().any(
                |event| matches!(event, ForegroundEvent::Input(input) if input.kind == "mouse_down")
            ));
            assert!(
                incident
                    .contributors
                    .iter()
                    .all(|event| event.duration() >= Duration::from_millis(1))
            );
        }
        let serialized = SerializedHangIncident::convert(
            start,
            incident,
            usize::MAX,
            detector.first_present_at(),
        );
        assert_eq!(serialized.measurement_version, 3);
        assert_eq!(serialized.trigger.as_str(), expected.as_str());
        assert!(serialized.during_active_use);
        let json = serde_json::to_value(serialized).unwrap();
        assert_eq!(
            json["trigger"],
            if measured {
                "late_frame"
            } else {
                "blocking_work"
            }
        );
    }
}
