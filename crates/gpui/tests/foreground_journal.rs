#![cfg(all(feature = "test-support", feature = "profiler"))]

use open_gpui::{
    Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton, Render,
    RequestFrameOptions, Styled as _, TestAppContext, VisualTestContext, Window, WindowHandle,
    WindowVisibility, div,
    profiler::{
        hang::{HangDetector, HangIncident, SerializedHangIncident},
        journal::{
            ForegroundEvent, ForegroundJournalEntry, FrameSnapshot, FrameStateChange,
            IntervalBoundary, IntervalSealer,
        },
    },
    px, size,
};
use open_gpui_scheduler::Instant;
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Default)]
struct ViewObservations {
    inputs: Cell<usize>,
    renders: Cell<usize>,
}

struct JournalView(Rc<ViewObservations>);

impl Render for JournalView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let observed = Rc::clone(&self.0);
        div()
            .size_full()
            .bg(if observed.inputs.get().is_multiple_of(2) {
                open_gpui::red()
            } else {
                open_gpui::blue()
            })
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                observed.inputs.set(observed.inputs.get() + 1);
                window.refresh();
            })
    }
}

fn open_window(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<JournalView>,
    VisualTestContext,
    Rc<ViewObservations>,
) {
    let observed = Rc::new(ViewObservations::default());
    cx.defer_next_window_frame_requests();
    let handle = cx.open_window(size(px(96.), px(96.)), {
        let observed = Rc::clone(&observed);
        move |_, _| JournalView(observed)
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, _| assert!(window.is_window_active()));
    present_frame(&mut visual);
    assert!(observed.renders.get() > 0);
    (handle, visual, observed)
}

fn present_frame(visual: &mut VisualTestContext) {
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
}

fn click(visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.dispatch_event(
            open_gpui::PlatformInput::MouseDown(open_gpui::MouseDownEvent {
                position: open_gpui::point(px(8.), px(8.)),
                button: MouseButton::Left,
                modifiers: Modifiers::default(),
                click_count: 1,
                ..Default::default()
            }),
            cx,
        );
    });
}

fn serialized_snapshots(start: Instant, snapshots: Vec<FrameSnapshot>) -> Vec<serde_json::Value> {
    snapshots
        .into_iter()
        .map(|snapshot| {
            let incident = HangIncident::detect(snapshot, Duration::ZERO, Duration::MAX)
                .expect("a sealed interval with work must meet the zero test threshold");
            serde_json::to_value(SerializedHangIncident::convert(
                start,
                &incident,
                usize::MAX,
                None,
            ))
            .expect("public incidents must serialize")
        })
        .collect()
}

#[open_gpui::test]
fn public_collectors_and_detector_observe_input_draw_and_present_independently(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    let window_id = handle.window_id();
    let start = Instant::now();
    let journal = cx.read(|app| app.foreground_journal());
    let mut immediate = journal.collector();
    let mut delayed = journal.clone().collector();
    let mut detector = HangDetector::new(journal.clone(), Duration::ZERO, Duration::MAX);
    let mut sealer = IntervalSealer::new(start);
    let initial_renders = observed.renders.get();
    assert!(immediate.collect_unseen().entries.is_empty());
    assert!(detector.poll().is_empty());
    assert_eq!(detector.first_present_at(), None);

    click(&mut visual);

    assert_eq!(observed.inputs.get(), 1);
    assert!(observed.renders.get() > initial_renders);
    let input = immediate.collect_unseen();
    assert_eq!(input.lost, 0);
    assert!(input.entries.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Event(ForegroundEvent::Draw(draw)) if draw.window_id == window_id
    )));
    assert!(!input.entries.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
    )));
    let pending_index = input
        .entries
        .iter()
        .position(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::FrameState(FrameStateChange::Pending { window_id: id, .. })
                    if *id == window_id
            )
        })
        .expect("the input handler's refresh must publish pending demand");
    let (input_index, input_timing) = input
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, entry)| match entry {
            ForegroundJournalEntry::Event(ForegroundEvent::Input(timing))
                if timing.kind == "mouse_down" && timing.caused_invalidation =>
            {
                Some((index, *timing))
            }
            _ => None,
        })
        .expect("the dispatched input must be retained after its handler returns");
    assert!(pending_index < input_index);
    let input_entry_count = input.entries.len();
    assert!(sealer.push_entries(input.entries).is_empty());
    assert!(immediate.collect_unseen().entries.is_empty());
    assert!(detector.poll().is_empty());
    let mut late = journal.collector();
    drop(journal);

    present_frame(&mut visual);

    assert!(observed.renders.get() > initial_renders);
    let frame = immediate.collect_unseen();
    assert_eq!(frame.lost, 0);
    let (draw_index, drawn) = frame
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, entry)| match entry {
            ForegroundJournalEntry::Event(ForegroundEvent::Draw(timing))
                if timing.window_id == window_id =>
            {
                Some((index, *timing))
            }
            _ => None,
        })
        .expect("the actual window render must complete a draw");
    let (present_index, presented) = frame
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, entry)| match entry {
            ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented))
                if presented.frame.window_id == window_id =>
            {
                Some((index, *presented))
            }
            _ => None,
        })
        .expect("the test platform frame callback must seal the accepted draw");
    assert!(draw_index < present_index);
    assert!(input_timing.end <= drawn.draw_start);
    assert_eq!(drawn.draw_start, presented.frame.draw_start);
    assert_eq!(drawn.draw_end, presented.frame.draw_end);
    assert_eq!(drawn.dirty_at, presented.frame.dirty_at);
    assert_eq!(presented.presentation.window_id, window_id);
    assert!(presented.frame.dirty_at.is_some());
    assert!(presented.frame.invalidations > 0);
    assert!(presented.frame.draw_end <= presented.presentation.present_start);
    let frame_entry_count = frame.entries.len();
    let snapshots = sealer.push_entries(frame.entries);
    let snapshot = snapshots
        .iter()
        .find(|snapshot| matches!(snapshot.boundary, IntervalBoundary::Presented(_)))
        .expect("a pending input must remain open until presentation");
    assert!(snapshot.events.iter().any(|event| {
        matches!(event, ForegroundEvent::Input(timing) if timing.kind == "mouse_down")
    }));

    let delayed_entries = delayed.collect_unseen();
    assert_eq!(delayed_entries.lost, 0);
    assert_eq!(
        delayed_entries.entries.len(),
        input_entry_count + frame_entry_count
    );
    let delayed_snapshots = IntervalSealer::new(start).push_entries(delayed_entries.entries);
    assert_eq!(
        serialized_snapshots(start, snapshots),
        serialized_snapshots(start, delayed_snapshots)
    );
    let late_entries = late.collect_unseen();
    assert_eq!(late_entries.lost, 0);
    assert_eq!(late_entries.entries.len(), frame_entry_count);
    assert!(!late_entries.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Input(_))
        )
    }));
    let incidents = detector.poll();
    let incident = incidents
        .iter()
        .find(|incident| {
            incident.snapshot.interval_end() == presented.presentation.present_end
                && incident.snapshot.events.iter().any(|event| {
                    matches!(event, ForegroundEvent::Input(timing) if timing.kind == "mouse_down")
                })
        })
        .expect("the detector must observe the same input and presented frame as both collectors");
    assert_eq!(
        detector.first_present_at(),
        Some(presented.presentation.present_end)
    );
    let json = serde_json::to_value(SerializedHangIncident::convert(
        start,
        incident,
        usize::MAX,
        detector.first_present_at(),
    ))
    .unwrap();
    assert_eq!(json["measurement_version"], 3);
    assert_eq!(json["sealed_by"], "present");
    assert_eq!(json["trigger"], "threshold");
    let contributors = json["contributors"].as_array().unwrap();
    assert!(contributors.iter().any(|contributor| {
        contributor["kind"] == "input"
            && contributor["input_kind"] == "mouse_down"
            && contributor["caused_invalidation"] == true
    }));
    for kind in ["draw", "present"] {
        assert!(contributors.iter().any(|contributor| {
            contributor["kind"] == kind
                && contributor["window_id"].as_u64() == Some(window_id.as_u64())
        }));
    }
    assert!(detector.poll().is_empty());
    for collector in [&mut immediate, &mut delayed, &mut late] {
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(drained.entries.is_empty());
    }
}

#[open_gpui::test]
fn public_visibility_releases_idle_and_rearms_the_same_pending_window(cx: &mut TestAppContext) {
    let (handle, mut visual, observed) = open_window(cx);
    let window_id = handle.window_id();
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    let mut sealer = IntervalSealer::new(Instant::now());
    click(&mut visual);
    let renders = observed.renders.get();
    let pending = collector.collect_unseen();
    assert_eq!(pending.lost, 0);
    assert!(sealer.push_entries(pending.entries).is_empty());

    visual.simulate_visibility_change(WindowVisibility::Hidden);
    visual.simulate_modifiers_change(Modifiers {
        shift: true,
        ..Modifiers::default()
    });

    visual.update(|window, _| assert!(!window.is_visible()));
    assert_eq!(observed.inputs.get(), 1);
    assert_eq!(observed.renders.get(), renders);
    let hidden = collector.collect_unseen();
    assert_eq!(hidden.lost, 0);
    assert!(!hidden.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::FrameState(FrameStateChange::Pending { .. })
                | ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(_))
        )
    }));
    let snapshots = sealer.push_entries(hidden.entries);
    assert!(snapshots.iter().any(|snapshot| {
        matches!(snapshot.boundary, IntervalBoundary::Idle { .. })
            && snapshot.events.iter().any(|event| {
                matches!(event, ForegroundEvent::Input(timing) if timing.kind == "mouse_down")
            })
    }));

    visual.simulate_visibility_change(WindowVisibility::Visible);
    visual.update(|window, _| assert!(window.is_visible()));
    let visible = collector.collect_unseen();
    assert_eq!(visible.lost, 0);
    assert!(visible.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::FrameState(FrameStateChange::Pending { window_id: id, .. })
                if *id == window_id
        )
    }));
    assert!(sealer.push_entries(visible.entries).is_empty());
    present_frame(&mut visual);
    assert!(observed.renders.get() > renders);
    let frame = collector.collect_unseen();
    assert_eq!(frame.lost, 0);
    assert!(
        sealer
            .push_entries(frame.entries)
            .iter()
            .any(|snapshot| matches!(
                snapshot.boundary,
                IntervalBoundary::Presented(presented) if presented.frame.window_id == window_id
            ))
    );
}

#[open_gpui::test]
fn public_power_boundaries_exclude_suspended_input_and_stale_dirty_timing(cx: &mut TestAppContext) {
    let (_, mut visual, observed) = open_window(cx);
    let mut collector = cx.read(|app| app.foreground_journal().collector());
    let mut sealer = IntervalSealer::new(Instant::now());
    click(&mut visual);
    let pending = collector.collect_unseen();
    assert_eq!(pending.lost, 0);
    assert!(sealer.push_entries(pending.entries).is_empty());

    cx.simulate_system_sleep();
    let sleep = collector.collect_unseen();
    assert_eq!(sleep.lost, 0);
    let snapshots = sealer.push_entries(sleep.entries);
    assert!(snapshots.iter().any(|snapshot| {
        matches!(snapshot.boundary, IntervalBoundary::PowerTransition { .. })
            && snapshot.events.iter().any(|event| {
                matches!(event, ForegroundEvent::Input(timing) if timing.kind == "mouse_down")
            })
    }));

    click(&mut visual);
    assert_eq!(observed.inputs.get(), 2);
    let suspended = collector.collect_unseen();
    assert_eq!(suspended.lost, 0);
    assert!(!suspended.entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::Input(_))
                | ForegroundJournalEntry::FrameState(FrameStateChange::Pending { .. })
        )
    }));
    assert!(sealer.push_entries(suspended.entries).is_empty());

    cx.simulate_system_wake();
    let wake = collector.collect_unseen();
    assert_eq!(wake.lost, 0);
    let wake_at = wake
        .entries
        .iter()
        .find_map(|entry| match entry {
            ForegroundJournalEntry::Boundary(IntervalBoundary::PowerTransition { ended_at }) => {
                Some(*ended_at)
            }
            _ => None,
        })
        .expect("the wake notification must interrupt the measurement epoch");
    sealer.push_entries(wake.entries);
    click(&mut visual);
    assert_eq!(observed.inputs.get(), 3);
    present_frame(&mut visual);
    let resumed = collector.collect_unseen();
    assert_eq!(resumed.lost, 0);
    let snapshots = sealer.push_entries(resumed.entries);
    let resumed = snapshots
        .iter()
        .find(|snapshot| matches!(snapshot.boundary, IntervalBoundary::Presented(_)))
        .expect("a fresh post-wake draw must complete its own interval");
    let inputs = resumed
        .events
        .iter()
        .filter_map(|event| match event {
            ForegroundEvent::Input(timing) if timing.kind == "mouse_down" => Some(timing),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].start >= wake_at);
    assert!(
        resumed
            .boundary
            .dirty_at()
            .is_none_or(|dirty_at| dirty_at >= wake_at)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn public_app_shutdown_flushes_the_monitor_and_drop_releases_its_callback() {
    use open_gpui::profiler::hang::{HangMonitorConfig, HangMonitorPollReason};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, TryRecvError},
    };

    let foreground_thread = std::thread::current().id();
    let completed = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let (mut survivor, app_lifetime, window_id) = {
        let mut cx = TestAppContext::single();
        cx.executor().allow_parking();
        let (handle, mut visual, observed) = open_window(&mut cx);
        let app_lifetime = Rc::downgrade(&cx.app);
        let window_id = handle.window_id();
        let view_lifetime = handle.entity(&cx).unwrap().downgrade();
        let mut collector = cx.read(|app| app.foreground_journal().collector());
        cx.update(|app| {
            let completed = Arc::clone(&completed);
            app.start_hang_monitor(
                HangMonitorConfig {
                    threshold: Duration::ZERO,
                    frame_budget: Duration::MAX,
                    interval: Duration::from_secs(3600),
                },
                move |poll| {
                    sender.send((poll, std::thread::current().id())).unwrap();
                    completed.store(true, Ordering::Release);
                },
            )
            .expect("the public App must own the monitor thread");
        });
        click(&mut visual);
        present_frame(&mut visual);
        assert_eq!(observed.inputs.get(), 1);
        let entries = collector.collect_unseen();
        assert_eq!(entries.lost, 0);
        let presented_at = entries
            .entries
            .iter()
            .find_map(|entry| match entry {
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(presented))
                    if presented.frame.window_id == window_id =>
                {
                    Some(presented.presentation.present_end)
                }
                _ => None,
            })
            .expect("the collector must independently observe the monitor's frame");
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

        cx.quit();

        assert!(completed.load(Ordering::Acquire));
        let (poll, callback_thread) = receiver
            .try_recv()
            .expect("shutdown must await the flush callback");
        assert_eq!(poll.reason, HangMonitorPollReason::Flush);
        assert_ne!(callback_thread, foreground_thread);
        assert_eq!(poll.first_present_at, Some(presented_at));
        assert!(poll.incidents.iter().any(|incident| {
            incident.snapshot.interval_end() == presented_at
                && incident.snapshot.events.iter().any(|event| {
                    matches!(event, ForegroundEvent::Input(timing) if timing.kind == "mouse_down")
                })
        }));
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        assert!(view_lifetime.upgrade().is_none());
        drop(visual);
        drop(cx);
        (collector, app_lifetime, window_id)
    };
    assert!(app_lifetime.upgrade().is_none());
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(2)),
        Err(RecvTimeoutError::Disconnected)
    ));
    let closed = survivor.collect_unseen();
    assert_eq!(closed.lost, 0);
    assert_eq!(
        closed
            .entries
            .iter()
            .filter(|entry| matches!(
                entry,
                ForegroundJournalEntry::FrameState(FrameStateChange::Closed { window_id: id, .. })
                    if *id == window_id
            ))
            .count(),
        1
    );
    assert!(survivor.collect_unseen().entries.is_empty());
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn public_monitor_rejects_invalid_intervals_without_consuming_the_app_slot() {
    use open_gpui::profiler::hang::{HangMonitorConfig, HangMonitorError};
    use std::sync::Arc;

    for interval in [Duration::ZERO, Duration::MAX] {
        let cx = TestAppContext::single();
        cx.executor().allow_parking();
        let callback_capture = Arc::new(());
        let retained = Arc::clone(&callback_capture);
        cx.update(|app| {
            let result = app.start_hang_monitor(
                HangMonitorConfig {
                    threshold: Duration::from_millis(100),
                    frame_budget: Duration::from_millis(24),
                    interval,
                },
                move |_| {
                    let _capture = &retained;
                },
            );
            assert!(matches!(result, Err(HangMonitorError::InvalidInterval)));
            assert_eq!(Arc::strong_count(&callback_capture), 1);
            app.start_hang_monitor(
                HangMonitorConfig {
                    threshold: Duration::from_millis(100),
                    frame_budget: Duration::from_millis(24),
                    interval: Duration::from_secs(3600),
                },
                |_| {},
            )
            .expect("a refused configuration must leave the App monitor slot usable");
        });
        cx.quit();
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn public_monitor_flush_survives_quit_observer_panics_and_reentry() {
    use open_gpui::profiler::hang::{HangMonitorConfig, HangMonitorPollReason};
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::mpsc,
    };

    for asynchronous_panic in [false, true] {
        let cx = TestAppContext::single();
        cx.executor().allow_parking();
        let callbacks = Rc::new(Cell::new(0));
        let (sender, receiver) = mpsc::channel();
        cx.update(|app| {
            app.start_hang_monitor(
                HangMonitorConfig {
                    threshold: Duration::from_millis(100),
                    frame_budget: Duration::from_millis(24),
                    interval: Duration::from_secs(3600),
                },
                move |poll| sender.send(poll.reason).unwrap(),
            )
            .unwrap();
            app.on_app_quit({
                let callbacks = Rc::clone(&callbacks);
                move |app| {
                    callbacks.set(callbacks.get() + 1);
                    app.shutdown();
                    assert!(
                        asynchronous_panic,
                        "injected synchronous quit observer panic"
                    );
                    async move {
                        panic!("injected asynchronous quit observer panic");
                    }
                }
            })
            .detach();
        });
        assert!(catch_unwind(AssertUnwindSafe(|| cx.quit())).is_err());
        assert_eq!(callbacks.get(), 1);
        assert_eq!(receiver.try_recv().unwrap(), HangMonitorPollReason::Flush);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn public_shutdown_does_not_wait_indefinitely_for_a_blocked_monitor_callback() {
    use open_gpui::profiler::hang::{HangMonitorConfig, HangMonitorPollReason};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    let cx = TestAppContext::single();
    cx.executor().allow_parking();
    let completed = Arc::new(AtomicBool::new(false));
    let callback_completed = Arc::clone(&completed);
    let (started, started_receiver) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (finished, finished_receiver) = mpsc::channel();
    cx.update(|app| {
        app.start_hang_monitor(
            HangMonitorConfig {
                threshold: Duration::from_millis(100),
                frame_budget: Duration::from_millis(24),
                interval: Duration::from_secs(3600),
            },
            move |poll| {
                assert_eq!(poll.reason, HangMonitorPollReason::Flush);
                started.send(()).unwrap();
                let _ = released.recv_timeout(Duration::from_secs(2));
                callback_completed.store(true, Ordering::Release);
                finished.send(()).unwrap();
            },
        )
        .unwrap();
    });
    cx.quit();
    started_receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(!completed.load(Ordering::Acquire));
    release.send(()).unwrap();
    finished_receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(completed.load(Ordering::Acquire));
}
