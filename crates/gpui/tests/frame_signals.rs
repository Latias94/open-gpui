#![cfg(all(feature = "test-support", feature = "profiler"))]

use open_gpui::{
    App, Context, FrameRequestSource, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, PlatformFrameSignal, PlatformInput, PlatformWindowPresentOutcome, Render,
    RequestFrameOptions, Styled as _, TestAppContext, VisualTestContext, Window, WindowHandle,
    WindowInitialPresentationStatus, WindowPresentationFacts, div, point,
    profiler::{
        hang::{HangDetector, HangIncident, SerializedHangIncident},
        journal::{
            ForegroundEvent, ForegroundJournalEntry, FrameSkipReason, FrameStateChange,
            IntervalBoundary, IntervalSealer,
        },
    },
    px, size,
};
use open_gpui_scheduler::Instant;
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    inputs: Cell<usize>,
    invalidate_on_input: Cell<bool>,
}

struct SignalView(Rc<Observations>);

impl Render for SignalView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let observed = Rc::clone(&self.0);
        div().size_full().bg(open_gpui::blue()).on_mouse_down(
            MouseButton::Left,
            move |_, window, _| {
                observed.inputs.set(observed.inputs.get() + 1);
                if observed.invalidate_on_input.get() {
                    window.refresh();
                }
            },
        )
    }
}

fn open_window(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<SignalView>,
    VisualTestContext,
    Rc<Observations>,
) {
    let observed = Rc::new(Observations::default());
    cx.defer_next_window_frame_requests();
    let handle = cx.open_window(size(px(96.), px(96.)), {
        let observed = Rc::clone(&observed);
        move |_, _| SignalView(observed)
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, _| assert!(window.is_window_active()));
    assert!(visual.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    let facts = presentation_facts(&mut visual);
    assert_eq!(
        facts.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert!(facts.native_visible);
    assert!(facts.frame_accepted_generation.is_some());
    assert_eq!(
        facts.frame_accepted_generation,
        facts.present_submitted_generation
    );
    assert_eq!(
        facts.non_empty_presented_generation,
        facts.present_submitted_generation
    );
    assert_eq!(
        facts.latest_present_attempt.unwrap().outcome,
        PlatformWindowPresentOutcome::Submitted
    );
    (handle, visual, observed)
}

fn presentation_facts(visual: &mut VisualTestContext) -> WindowPresentationFacts {
    visual.update(|window, _| window.presentation_facts())
}

fn dispatch_input(window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(8.), px(8.)),
            button: MouseButton::Left,
            ..Default::default()
        }),
        cx,
    );
}

#[open_gpui::test]
fn public_frame_sources_precede_skips_that_seal_real_input_without_drawing(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    let window_id = handle.window_id();
    let baseline = presentation_facts(&mut visual);
    let renders = observed.renders.get();
    let start = Instant::now();
    let journal = cx.read(|app| app.foreground_journal());
    let mut collector = journal.collector();
    let mut detector = HangDetector::new(journal, Duration::ZERO, Duration::MAX);

    for (index, (source, captured)) in [
        (FrameRequestSource::NativeCallback, true),
        (FrameRequestSource::LocalSchedule, true),
        (FrameRequestSource::NativeCallback, false),
        (FrameRequestSource::LocalSchedule, false),
    ]
    .into_iter()
    .enumerate()
    {
        let (signal_at, source) = if captured {
            let signals = PlatformFrameSignal::new();
            let at = Instant::now();
            let other_source = match source {
                FrameRequestSource::NativeCallback => FrameRequestSource::LocalSchedule,
                FrameRequestSource::LocalSchedule => FrameRequestSource::NativeCallback,
            };
            signals.record(at + Duration::from_millis(1), other_source);
            signals.record(at, source);
            let (captured_at, captured_source) = signals.take().unwrap();
            assert_eq!((captured_at, captured_source), (at, source));
            assert_eq!(signals.take(), None);
            (Some(captured_at), captured_source)
        } else {
            (None, source)
        };
        visual.update(|window, _| window.on_next_frame(dispatch_input));

        assert!(visual.simulate_frame(RequestFrameOptions {
            signal_at,
            signal_source: source,
            ..Default::default()
        }));

        assert_eq!(observed.inputs.get(), index + 1);
        assert_eq!(observed.renders.get(), renders);
        assert_eq!(presentation_facts(&mut visual), baseline);
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert_eq!(
            drained
                .entries
                .iter()
                .filter(|entry| matches!(entry, ForegroundJournalEntry::PlatformSignal(_)))
                .count(),
            1,
            "foreground delivery must not repeat the callback's raw marker"
        );
        let (signal_index, signal) = drained
            .entries
            .iter()
            .enumerate()
            .find_map(|(index, entry)| match entry {
                ForegroundJournalEntry::PlatformSignal(signal) => Some((index, *signal)),
                _ => None,
            })
            .expect("the platform callback must publish its raw source marker");
        assert_eq!(signal.window_id, window_id);
        assert_eq!(signal.signal_at, signal_at);
        assert_eq!(signal.source, source);
        assert!(signal_at.is_none_or(|at| signal.handled_at >= at));
        let input_index = drained
            .entries
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    ForegroundJournalEntry::Event(ForegroundEvent::Input(input))
                        if input.kind == "mouse_down" && !input.caused_invalidation
                )
            })
            .expect("the next-frame callback must dispatch actual noninvalidating input");
        let skip_index = drained
            .entries
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(skipped))
                        if skipped.window_id == window_id
                            && skipped.reason == FrameSkipReason::NoRenderNeeded
                            && skipped.at >= signal.handled_at
                )
            })
            .expect("completion without a draw must seal the retained input");
        assert!(signal_index < input_index);
        assert!(input_index < skip_index);
        assert!(drained.entries.iter().all(|entry| {
            !matches!(
                entry,
                ForegroundJournalEntry::Event(
                    ForegroundEvent::Draw(_) | ForegroundEvent::Present(_)
                ) | ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
            )
        }));

        let mut sealer = IntervalSealer::new(start);
        assert!(
            sealer
                .push_entries(drained.entries[..skip_index].iter().copied())
                .is_empty()
        );
        assert!(
            sealer
                .push_entries([ForegroundJournalEntry::ReentrantFrameSkipped {
                    window_id,
                    at: signal.handled_at,
                }])
                .is_empty()
        );
        let snapshots = sealer.push_entries(drained.entries[skip_index..].iter().copied());
        let snapshot = snapshots
            .into_iter()
            .find(|snapshot| matches!(snapshot.boundary, IntervalBoundary::FrameSkipped(_)))
            .expect("raw signals and reentrant markers must leave input for the completed skip");
        assert!(snapshot.boundary.dirty_at().is_none());
        let incident = HangIncident::detect(snapshot, Duration::ZERO, Duration::MAX).unwrap();
        let json = serde_json::to_value(SerializedHangIncident::convert(
            start,
            &incident,
            usize::MAX,
            None,
        ))
        .unwrap();
        assert_eq!(json["sealed_by"], "frame_skipped");
        assert!(json["dirty_to_present_ms"].is_null());
        assert!(
            json["contributors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| { entry["kind"] == "input" && entry["input_kind"] == "mouse_down" })
        );
        assert!(detector.poll().iter().any(|incident| {
            matches!(
                incident.snapshot.boundary,
                IntervalBoundary::FrameSkipped(skipped) if skipped.window_id == window_id
            )
        }));
        assert_eq!(detector.first_present_at(), None);
    }
}

#[open_gpui::test]
fn public_idle_frame_callbacks_do_not_draw_seal_empty_intervals_or_report_incidents(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    let baseline = presentation_facts(&mut visual);
    let renders = observed.renders.get();
    let start = Instant::now();
    let journal = cx.read(|app| app.foreground_journal());
    let mut collector = journal.collector();
    let mut detector = HangDetector::new(journal, Duration::ZERO, Duration::ZERO);
    let mut sealer = IntervalSealer::new(start);

    for index in 0..8 {
        let source = if index % 2 == 0 {
            FrameRequestSource::NativeCallback
        } else {
            FrameRequestSource::LocalSchedule
        };
        let signal_at = (index % 3 != 0).then(Instant::now);
        assert!(visual.simulate_frame(RequestFrameOptions {
            signal_at,
            signal_source: source,
            ..Default::default()
        }));
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(matches!(
            drained.entries.as_slice(),
            [ForegroundJournalEntry::PlatformSignal(signal)]
                if signal.window_id == handle.window_id()
                    && signal.signal_at == signal_at
                    && signal.source == source
        ));
        assert!(sealer.push_entries(drained.entries).is_empty());
        assert!(detector.poll().is_empty());
    }

    assert_eq!(observed.inputs.get(), 0);
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(presentation_facts(&mut visual), baseline);
    assert_eq!(detector.first_present_at(), None);
}

#[open_gpui::test]
fn public_no_draw_skip_preserves_a_deferred_scene_until_close_and_native_retirement(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(open_gpui::QuitMode::Explicit));
    let (handle, mut visual, observed) = open_window(cx);
    let (_, mut idle_probe, idle_observed) = open_window(cx);
    let idle_renders = idle_observed.renders.get();
    let window_id = handle.window_id();
    visual.deactivate_window();
    visual.update(|window, _| assert!(!window.is_window_active()));
    let baseline = presentation_facts(&mut visual);
    let lifetime = handle.entity(cx).unwrap().downgrade();
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    let mut sealer = IntervalSealer::new(Instant::now());
    cx.set_window_present_outcome(handle.into(), PlatformWindowPresentOutcome::Deferred);
    observed.invalidate_on_input.set(true);
    visual.update(dispatch_input);
    let pending_facts = presentation_facts(&mut visual);
    let renders = observed.renders.get();
    assert!(pending_facts.frame_accepted_generation > baseline.frame_accepted_generation);
    assert_eq!(
        pending_facts.present_submitted_generation,
        baseline.present_submitted_generation
    );
    let pending = collector.collect_unseen();
    assert_eq!(pending.lost, 0);
    assert!(sealer.push_entries(pending.entries).is_empty());

    let signal_at = Some(Instant::now());
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        signal_at,
        signal_source: FrameRequestSource::NativeCallback,
        ..Default::default()
    }));
    assert_eq!(observed.renders.get(), renders);
    let deferred = presentation_facts(&mut visual);
    assert_eq!(
        deferred.frame_accepted_generation,
        pending_facts.frame_accepted_generation
    );
    assert_eq!(
        deferred.present_submitted_generation,
        baseline.present_submitted_generation
    );
    let attempt = deferred.latest_present_attempt.unwrap();
    assert_eq!(
        Some(attempt.generation),
        pending_facts.frame_accepted_generation
    );
    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Deferred);
    let skipped = collector.collect_unseen();
    assert_eq!(skipped.lost, 0);
    assert!(skipped.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::PlatformSignal(signal)
                if signal.window_id == window_id
                    && signal.signal_at == signal_at
                    && signal.source == FrameRequestSource::NativeCallback
        )
    }));
    let snapshots = sealer.push_entries(skipped.entries);
    assert!(snapshots.iter().any(|snapshot| {
        matches!(
            snapshot.boundary,
            IntervalBoundary::FrameSkipped(skipped)
                if skipped.window_id == window_id
                    && skipped.reason == FrameSkipReason::NoRenderNeeded
        ) && snapshot.events.iter().any(
            |event| matches!(event, ForegroundEvent::Input(input) if input.caused_invalidation),
        )
    }));

    observed.invalidate_on_input.set(false);
    visual.update(dispatch_input);
    let still_pending = collector.collect_unseen();
    assert_eq!(still_pending.lost, 0);
    assert!(still_pending.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Input(input))
                if !input.caused_invalidation
        )
    }));
    assert!(
        still_pending
            .entries
            .iter()
            .all(|entry| !matches!(entry, ForegroundJournalEntry::Boundary(_)))
    );
    assert!(sealer.push_entries(still_pending.entries).is_empty());

    let terminal = cx.hold_window_native_terminal(handle.into());
    assert!(cx.simulate_window_close(handle.into()));
    assert!(!cx.windows().contains(&handle.into()));
    assert!(lifetime.upgrade().is_none());
    idle_probe.update(dispatch_input);
    assert_eq!(idle_observed.inputs.get(), 1);
    assert_eq!(idle_observed.renders.get(), idle_renders);
    let closed = collector.collect_unseen();
    assert_eq!(closed.lost, 0);
    assert_eq!(
        closed
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    ForegroundJournalEntry::FrameState(
                        FrameStateChange::Closed { window_id: id, .. }
                    ) if *id == window_id
                )
            })
            .count(),
        1
    );
    assert!(
        sealer
            .push_entries(closed.entries)
            .iter()
            .any(|snapshot| matches!(snapshot.boundary, IntervalBoundary::Idle { .. }))
    );
    assert!(terminal.release());
    cx.run_until_parked();
    assert!(!cx.windows().contains(&handle.into()));
    let retired = collector.collect_unseen();
    assert_eq!(retired.lost, 0);
    assert!(retired.entries.iter().all(|entry| {
        !matches!(
            entry,
            ForegroundJournalEntry::FrameState(FrameStateChange::Closed { window_id: id, .. })
                if *id == window_id
        )
    }));
}

#[open_gpui::test]
fn public_inactive_throttling_retains_pending_input_callback_and_force_render(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    let window_id = handle.window_id();
    let start = Instant::now();
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    let mut sealer = IntervalSealer::new(start);
    let callback_count = Rc::new(Cell::new(0));

    for (completed_callbacks, require_presentation) in [true, false].into_iter().enumerate() {
        visual.deactivate_window();
        visual.update(|window, _| assert!(!window.is_window_active()));
        assert!(visual.simulate_frame(RequestFrameOptions::default()));
        let primed = collector.collect_unseen();
        assert_eq!(primed.lost, 0);
        sealer.push_entries(primed.entries);
        let baseline = presentation_facts(&mut visual);
        observed.invalidate_on_input.set(true);
        visual.update(|window, cx| {
            dispatch_input(window, cx);
            window.on_next_frame({
                let callback_count = Rc::clone(&callback_count);
                move |_, _| {
                    callback_count.set(callback_count.get() + 1);
                }
            });
        });
        let renders = observed.renders.get();
        let pending_facts = presentation_facts(&mut visual);
        let pending = collector.collect_unseen();
        let signal_at = Some(Instant::now());
        assert!(visual.simulate_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: false,
            signal_at,
            signal_source: FrameRequestSource::LocalSchedule,
        }));
        assert_eq!(callback_count.get(), completed_callbacks);
        assert_eq!(observed.renders.get(), renders);
        assert_eq!(presentation_facts(&mut visual), pending_facts);
        assert!(pending_facts.frame_accepted_generation > baseline.frame_accepted_generation);
        assert_eq!(
            pending_facts.present_submitted_generation,
            baseline.present_submitted_generation
        );
        assert_eq!(pending.lost, 0);
        assert!(pending.entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::FrameState(FrameStateChange::Pending { window_id: id, .. })
                    if *id == window_id
            )
        }));
        assert!(sealer.push_entries(pending.entries).is_empty());
        let skipped = collector.collect_unseen();
        assert_eq!(skipped.lost, 0);
        assert!(skipped.entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::PlatformSignal(signal)
                    if signal.window_id == window_id
                        && signal.signal_at == signal_at
                        && signal.source == FrameRequestSource::LocalSchedule
            )
        }));
        let snapshots = sealer.push_entries(skipped.entries);
        assert!(snapshots.iter().any(|snapshot| {
            matches!(
                snapshot.boundary,
                IntervalBoundary::FrameSkipped(skipped)
                    if skipped.window_id == window_id
                        && skipped.reason == FrameSkipReason::InactiveFrameRateLimit
            ) && snapshot.events.iter().any(
                |event| matches!(event, ForegroundEvent::Input(input) if input.caused_invalidation),
            )
        }));

        observed.invalidate_on_input.set(false);
        visual.update(dispatch_input);
        let still_pending = collector.collect_unseen();
        assert_eq!(still_pending.lost, 0);
        assert!(still_pending.entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::Event(ForegroundEvent::Input(input))
                    if !input.caused_invalidation
            )
        }));
        assert!(
            still_pending
                .entries
                .iter()
                .all(|entry| !matches!(entry, ForegroundJournalEntry::Boundary(_)))
        );
        assert!(sealer.push_entries(still_pending.entries).is_empty());

        assert!(visual.simulate_frame(RequestFrameOptions::default()));
        assert_eq!(callback_count.get(), completed_callbacks);
        assert_eq!(observed.renders.get(), renders);
        assert_eq!(presentation_facts(&mut visual), pending_facts);
        let ordinary_skip = collector.collect_unseen();
        assert_eq!(ordinary_skip.lost, 0);
        assert!(
            sealer
                .push_entries(ordinary_skip.entries)
                .iter()
                .any(|snapshot| {
                    matches!(
                        snapshot.boundary,
                        IntervalBoundary::FrameSkipped(skipped)
                            if skipped.window_id == window_id
                                && skipped.reason == FrameSkipReason::InactiveFrameRateLimit
                    ) && snapshot.events.iter().any(|event| {
                        matches!(event, ForegroundEvent::Input(input) if !input.caused_invalidation)
                    })
                })
        );

        if !require_presentation {
            let _activation = visual.update(|window, _| window.activate_window());
            visual.run_until_parked();
            visual.update(|window, _| assert!(window.is_window_active()));
        }
        let before_resume = presentation_facts(&mut visual);
        let renders = observed.renders.get();
        let options = if require_presentation {
            RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                signal_at: Some(Instant::now()),
                signal_source: FrameRequestSource::NativeCallback,
            }
        } else {
            RequestFrameOptions::default()
        };
        assert!(visual.simulate_frame(options));
        if require_presentation {
            visual.update(|window, _| assert!(!window.is_window_active()));
        }
        assert_eq!(callback_count.get(), completed_callbacks + 1);
        assert_eq!(observed.renders.get(), renders + 1);
        let submitted = presentation_facts(&mut visual);
        assert!(submitted.frame_accepted_generation > before_resume.frame_accepted_generation);
        assert_eq!(
            submitted.frame_accepted_generation,
            submitted.present_submitted_generation
        );
        assert_eq!(
            submitted.non_empty_presented_generation,
            submitted.present_submitted_generation
        );
        let attempt = submitted.latest_present_attempt.unwrap();
        assert_eq!(
            Some(attempt.generation),
            submitted.present_submitted_generation
        );
        assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
        let resumed = collector.collect_unseen();
        assert_eq!(resumed.lost, 0);
        assert!(
            sealer
                .push_entries(resumed.entries)
                .iter()
                .any(|snapshot| matches!(
                    snapshot.boundary,
                    IntervalBoundary::Presented(presented) if presented.frame.window_id == window_id
                ))
        );

        assert!(visual.simulate_frame(RequestFrameOptions::default()));
        assert_eq!(callback_count.get(), completed_callbacks + 1);
        assert_eq!(observed.renders.get(), renders + 1);
        assert_eq!(presentation_facts(&mut visual), submitted);
        let idle = collector.collect_unseen();
        assert_eq!(idle.lost, 0);
        assert!(matches!(
            idle.entries.as_slice(),
            [ForegroundJournalEntry::PlatformSignal(signal)]
                if signal.window_id == window_id
                    && signal.signal_at.is_none()
                    && signal.source == FrameRequestSource::NativeCallback
        ));
        assert!(sealer.push_entries(idle.entries).is_empty());
    }
}
