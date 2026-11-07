//! Bounded unified-profiler coverage using actual Windows windows and renderer submissions.
//!
//! Synthetic PlatformInput values enter the production native pointer/key callback and rendered
//! listeners. Paint receipts identify the source and candidate accepted by the real renderer.
//! Window creation and bounded message pumping qualify trace ordering around native submission.
//! This does not qualify OS input injection, compositor scanout, physical display changes, or
//! performance overhead.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use open_gpui::profiler::{
    hang::{HangMonitorConfig, HangMonitorPollReason},
    journal::{
        ForegroundEvent, ForegroundJournalCollector, ForegroundJournalEntry, IntervalBoundary,
        PlatformSignal,
    },
};
use open_gpui::{
    FocusHandle, FrameDurationSnapshot, FrameEvent, FrameRequestSource, FrameTiming,
    FrameTimingCollector, InputLatencySnapshot, InteractiveElement, KeyDownEvent, Keystroke,
    MouseButton, MouseDownEvent, MouseUpEvent, RequestFrameOptions, WindowHandle,
    WindowInitialPresentationStatus, fill, profiler, rgb,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SW_HIDE, SW_RESTORE, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE,
};

struct RestoreNativeProfilerTrace(bool);

impl Drop for RestoreNativeProfilerTrace {
    fn drop(&mut self) {
        profiler::set_trace_enabled(self.0);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeProfilerSource {
    pointer_inputs: u32,
    key_inputs: u32,
    redraws: u32,
}

#[derive(Clone, Debug)]
struct NativeProfilerPaint {
    window_id: WindowId,
    source: NativeProfilerSource,
    previous_revision: u64,
    painted_at: Instant,
    pending_inputs: u64,
}

#[derive(Clone, Copy, Debug)]
struct NativeProfilerSignal {
    options: RequestFrameOptions,
    entered_at: Instant,
}

struct NativeProfilerRoot {
    focus: FocusHandle,
    source: NativeProfilerSource,
    paints: Rc<RefCell<Vec<NativeProfilerPaint>>>,
}

impl Render for NativeProfilerRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = self.source;
        let paints = self.paints.clone();
        div()
            .id("native-profiler-consumer")
            .track_focus(&self.focus)
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|root, _, window, cx| {
                    root.source.pointer_inputs += 1;
                    for _ in 0..2 {
                        window.refresh();
                    }
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
                assert_eq!(event.keystroke.key, "p");
                root.source.key_inputs += 1;
                for _ in 0..2 {
                    window.refresh();
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        let color = rgb(0x285c80
                            + source.pointer_inputs * 0x100
                            + source.key_inputs * 0x10000
                            + source.redraws);
                        window.paint_quad(fill(bounds, color));
                        paints.borrow_mut().push(NativeProfilerPaint {
                            window_id: window.window_handle().window_id(),
                            source,
                            previous_revision: window.rendered_frame_revision(),
                            painted_at: Instant::now(),
                            pending_inputs: window.input_latency_snapshot().pending_input_count,
                        });
                    },
                )
                .size_full(),
            )
    }
}

struct NativeProfilerWindow {
    handle: WindowHandle<NativeProfilerRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    paints: Rc<RefCell<Vec<NativeProfilerPaint>>>,
    signals: Rc<RefCell<Vec<NativeProfilerSignal>>>,
    operations: RefCell<Vec<(Instant, Instant)>>,
}

impl NativeProfilerWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let started_at = Instant::now();
        let paints = Rc::new(RefCell::new(Vec::new()));
        let handle = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.), px(200.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let focus = cx.focus_handle();
                            window.focus(&focus, cx);
                            NativeProfilerRoot {
                                focus,
                                source: NativeProfilerSource::default(),
                                paints: paints.clone(),
                            }
                        })
                    },
                )
            })
            .expect("the native profiler consumer must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("the profiler consumer must own its exact HWND registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("the profiler consumer HWND must have a native owner");
        assert_eq!(native.registration, registration);
        let signals = Rc::new(RefCell::new(Vec::new()));
        let callback_signals = signals.clone();
        let request_frame = native
            .state
            .callbacks
            .request_frame
            .take()
            .expect("the native profiler consumer must retain its production frame callback");
        native
            .state
            .callbacks
            .request_frame
            .set(Some(Rc::new(move |options| {
                callback_signals.borrow_mut().push(NativeProfilerSignal {
                    options,
                    entered_at: Instant::now(),
                });
                request_frame(options);
            })));
        Self {
            handle,
            registration,
            native,
            paints,
            signals,
            operations: RefCell::new(vec![(started_at, Instant::now())]),
        }
    }

    fn snapshot(&self, app: &mut Application) -> (InputLatencySnapshot, FrameDurationSnapshot) {
        app.update_for_test(|cx| {
            self.handle.update(cx, |_, window, _| {
                (
                    window.input_latency_snapshot(),
                    window.frame_duration_snapshot(),
                )
            })
        })
        .expect("the profiler consumer must remain live while reading snapshots")
    }

    fn wait_for_submission(
        &self,
        app: &mut Application,
        source: NativeProfilerSource,
        previous_generation: u64,
    ) -> (u64, NativeProfilerPaint) {
        let started_at = Instant::now();
        let mut submitted = None;
        pump_messages_until_with_delay(
            "native profiler accepted nonempty Submitted frame",
            Duration::from_millis(5),
            || {
                let (facts, revision, diagnostics) = app
                    .update_for_test(|cx| {
                        self.handle.update(cx, |_, window, _| {
                            (
                                window.presentation_facts(),
                                window.rendered_frame_revision(),
                                window
                                    .submitted_frame_diagnostics_for_test()
                                    .map(|diagnostics| diagnostics.generation()),
                            )
                        })
                    })
                    .expect("the profiler consumer must survive through submission");
                let Some(attempt) = facts.latest_present_attempt else {
                    return false;
                };
                let paints = self.paints.borrow();
                let Some(paint) = paints.last() else {
                    return false;
                };
                if paint.window_id == self.handle.window_id()
                    && paint.source == source
                    && paint.previous_revision.checked_add(1) == Some(attempt.generation)
                    && attempt.generation > previous_generation
                    && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                    && attempt.contained_valid_primitives
                    && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                    && facts.native_visible
                    && facts.frame_accepted_generation == Some(attempt.generation)
                    && facts.present_submitted_generation == Some(attempt.generation)
                    && facts.non_empty_presented_generation == Some(attempt.generation)
                    && revision == attempt.generation
                    && diagnostics == Some(attempt.generation)
                {
                    submitted = Some((attempt.generation, paint.clone()));
                    true
                } else {
                    false
                }
            },
        );
        self.operations
            .borrow_mut()
            .push((started_at, Instant::now()));
        submitted.expect("the actual painted source must belong to the exact Submitted frame")
    }

    fn input(&self, app: &mut Application) -> (Instant, Instant) {
        let started_at = Instant::now();
        assert!(
            !self
                .native
                .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(40.), px(40.)),
                    click_count: 1,
                    ..Default::default()
                }))
                .propagate
        );
        self.native
            .dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(40.), px(40.)),
                click_count: 1,
                ..Default::default()
            }));
        assert!(
            !self
                .native
                .dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse("p")
                        .expect("valid native profiler keystroke")
                        .with_simulated_ime(),
                    is_held: false,
                    prefer_character_input: false,
                }))
                .propagate
        );
        let finished_at = Instant::now();
        self.operations.borrow_mut().push((started_at, finished_at));
        let (input, _) = self.snapshot(app);
        assert_eq!(input.pending_input_count, 3);
        assert!(input.latency_histogram.is_empty());
        assert!(input.events_per_frame_histogram.is_empty());
        (started_at, finished_at)
    }

    fn settle(&self, app: &mut Application) {
        let started_at = Instant::now();
        let mut quiet_since = started_at;
        let mut previous = None;
        pump_messages_until_with_delay(
            "native profiler input and frame completion settle",
            Duration::from_millis(5),
            || {
                let (input, frame) = self.snapshot(app);
                let current = (
                    input.pending_input_count,
                    input.latency_histogram.len(),
                    frame.draw_duration_histogram.len(),
                    frame.dirty_to_present_histogram.len(),
                );
                if previous != Some(current) {
                    quiet_since = Instant::now();
                    previous = Some(current);
                }
                input.pending_input_count == 0
                    && quiet_since.elapsed() >= Duration::from_millis(100)
            },
        );
        self.operations
            .borrow_mut()
            .push((started_at, Instant::now()));
    }

    fn retire(
        self,
        app: &mut Application,
        platform: &WindowsPlatform,
        observation: &NativeWindowTestObservation,
    ) {
        let hwnd = self.registration.as_raw();
        let lifetime = Rc::downgrade(&self.native);
        assert!(
            platform
                .raw_window_handles
                .read()
                .contains(&self.registration)
        );
        let renderer_borrow = self.native.state.renderer.borrow_mut();
        app.update_for_test(|cx| {
            self.handle
                .update(cx, |_, window, cx| window.remove_window(cx))
        })
        .expect("the exact native profiler consumer must close");
        assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
        assert!(
            platform
                .raw_window_handles
                .read()
                .contains(&self.registration)
        );
        assert!(self.native.state.frame_signal.take().is_none());
        platform.raw_window_handles.record_frame_signal(
            self.registration,
            Some(Instant::now()),
            FrameRequestSource::NativeCallback,
        );
        assert!(
            self.native.state.frame_signal.take().is_none(),
            "shutdown claim must reject late VSync capture before native retirement"
        );
        let callbacks_before_retirement = self.signals.borrow().len();
        self.native.request_frame(RequestFrameOptions {
            signal_source: FrameRequestSource::LocalSchedule,
            ..RequestFrameOptions::default()
        });
        unsafe {
            SendMessageW(
                hwnd,
                crate::WM_GPUI_FORCE_UPDATE_WINDOW,
                Some(WPARAM(self.registration.generation())),
                None,
            );
        }
        assert_eq!(self.signals.borrow().len(), callbacks_before_retirement);
        drop(renderer_borrow);
        pump_messages_until_with_delay(
            "exact native profiler registration retirement",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(hwnd)).as_bool() }
                    && !platform
                        .raw_window_handles
                        .read()
                        .contains(&self.registration)
                    && platform.inner.native_retirement_is_settled()
            },
        );
        let lifecycle = observation
            .events()
            .into_iter()
            .filter(|event| {
                event.window().window_id() == self.registration.window_id()
                    && event.window().native_generation() == self.registration.generation()
            })
            .filter_map(|event| match event.kind() {
                NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                    assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
                    None
                }
                lifecycle => Some(lifecycle),
            })
            .collect::<Vec<_>>();
        let [
            NativeWindowTestEventKind::PresentationQuiesced {
                ticket_generation: quiesced,
            },
            NativeWindowTestEventKind::DestroyEntered {
                ticket_generation: destroyed,
            },
            NativeWindowTestEventKind::NativeTerminal {
                ticket_generation: terminal,
            },
        ] = lifecycle.as_slice()
        else {
            panic!("unexpected native profiler retirement sequence: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
        assert!(app.update_for_test(|cx| {
            assert!(self.handle.update(cx, |_, _, _| ()).is_err());
            cx.windows().is_empty()
        }));
        assert!(platform.raw_window_handles.read().is_empty());
        drop(self);
        assert!(lifetime.upgrade().is_none());
    }
}

fn assert_native_profiler_outcomes(input: &InputLatencySnapshot) {
    assert!(input.drawn_frame_count > 0);
    assert!(input.submitted_present_count > 0);
    assert_eq!(
        input.present_attempt_count,
        input.submitted_present_count
            + input.deferred_present_count
            + input.repaint_required_present_count
            + input.rejected_present_count
    );
    assert_eq!(input.pending_input_count, 0);
    assert_eq!(input.mid_draw_events_dropped, 0);
}

fn assert_native_profiler_trace(
    consumer: &NativeProfilerWindow,
    collector: &mut FrameTimingCollector,
    submitted_generation: u64,
    paint: &NativeProfilerPaint,
    frame: &FrameDurationSnapshot,
) -> open_gpui::PresentTiming {
    let events = collector.collect_unseen();
    assert!(
        !events.is_empty(),
        "the Submitted source must produce trace events"
    );
    let mut pending_draw: Option<(FrameTiming, NativeProfilerPaint)> = None;
    let mut matching_present = None;
    let mut previous_candidate = None;
    for event in &events {
        match event {
            FrameEvent::Draw(draw) => {
                assert_eq!(draw.window_id, consumer.handle.window_id());
                let paints = consumer.paints.borrow();
                let source = paints
                    .iter()
                    .find(|receipt| {
                        receipt.window_id == draw.window_id
                            && draw.draw_start <= receipt.painted_at
                            && receipt.painted_at <= draw.draw_end
                    })
                    .expect("each traced draw must contain an actual native paint receipt");
                if let Some(previous) = previous_candidate {
                    assert!(source.previous_revision > previous);
                }
                previous_candidate = Some(source.previous_revision);
                let dirty_at = draw
                    .dirty_at
                    .expect("the explicit redraw must have a dirty timestamp");
                assert!(draw.invalidations > 0);
                assert!(dirty_at <= draw.draw_start);
                assert!(draw.draw_start <= draw.draw_end);
                let operations = consumer.operations.borrow();
                assert!(
                    operations
                        .iter()
                        .any(|(start, end)| { *start <= draw.draw_start && draw.draw_end <= *end })
                );
                let draw_nanos =
                    u64::try_from(draw.draw_duration().as_nanos()).expect("bounded native draw");
                assert!(frame.draw_duration_histogram.count_at(draw_nanos) > 0);
                if let Some((superseded, _)) = pending_draw.replace((*draw, source.clone())) {
                    assert!(superseded.draw_end <= draw.draw_start);
                }
            }
            FrameEvent::Present(present) => {
                let (draw, source) = pending_draw
                    .take()
                    .expect("a submission must follow its latest accepted draw");
                assert_eq!(present.window_id, draw.window_id);
                assert!(draw.draw_end <= present.present_start);
                assert!(present.present_start <= present.present_end);
                let operations = consumer.operations.borrow();
                assert!(operations.iter().any(|(start, end)| {
                    *start <= present.present_start && present.present_end <= *end
                }));
                let dirty_at = draw
                    .dirty_at
                    .expect("the submitted draw must have a dirty timestamp");
                let dirty_nanos =
                    u64::try_from(present.present_end.duration_since(dirty_at).as_nanos())
                        .expect("bounded native dirty-to-present duration");
                assert!(frame.dirty_to_present_histogram.count_at(dirty_nanos) > 0);
                if source.painted_at == paint.painted_at {
                    assert_eq!(source.window_id, paint.window_id);
                    assert_eq!(source.source, paint.source);
                    assert_eq!(source.previous_revision, paint.previous_revision);
                    assert_eq!(
                        source.previous_revision.checked_add(1),
                        Some(submitted_generation)
                    );
                    assert!(matching_present.replace(*present).is_none());
                }
            }
        }
    }
    assert!(
        pending_draw.is_none(),
        "the latest accepted draw must have a submission"
    );
    assert!(collector.collect_unseen().is_empty());
    matching_present.expect("the exact accepted nonempty source must have a traced submission")
}

#[test]
fn native_profiler_tracks_input_submission_redraw_and_reconstructed_window() {
    discard_stale_quit_messages();
    let _restore_trace = RestoreNativeProfilerTrace(profiler::trace_enabled());
    profiler::set_trace_enabled(false);
    assert!(!profiler::trace_enabled());
    let mut collector = FrameTimingCollector::new();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new(false).expect("the native profiler Windows backend must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let journal = app.update_for_test(|cx| cx.foreground_journal());
    let mut journal_collector = journal.collector();
    let mut independent_collector = journal.collector();
    let callback_lifetime = Arc::new(());
    let callback_weak = Arc::downgrade(&callback_lifetime);
    let monitor_completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_completed = Arc::clone(&monitor_completed);
    let (monitor_sender, monitor_receiver) = std::sync::mpsc::channel();
    let foreground_thread = std::thread::current().id();
    app.update_for_test(|cx| {
        cx.start_hang_monitor(
            HangMonitorConfig {
                threshold: Duration::from_millis(100),
                frame_budget: Duration::from_millis(24),
                interval: Duration::from_secs(3600),
            },
            move |poll| {
                let _keep_alive = &callback_lifetime;
                monitor_sender
                    .send((
                        poll.reason,
                        std::thread::current().id(),
                        poll.first_present_at,
                    ))
                    .expect("the native consumer must retain its monitor receiver");
                callback_completed.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .expect("the native consumer must start the opt-in GPUI hang monitor");
    });
    platform.begin_vsync_thread();
    let consumer = NativeProfilerWindow::open(&mut app, &platform);
    let (mut generation, _) =
        consumer.wait_for_submission(&mut app, NativeProfilerSource::default(), 0);
    pump_messages_until_idle("native profiler initial submission follow-up");
    let (initial_input, initial_frame) = consumer.snapshot(&mut app);
    assert_native_profiler_outcomes(&initial_input);
    assert!(initial_input.latency_histogram.is_empty());
    assert!(!initial_frame.draw_duration_histogram.is_empty());
    assert!(initial_frame.present_interval_histogram.is_empty());
    assert!(collector.collect_unseen().is_empty());
    app.update_for_test(|cx| {
        consumer
            .handle
            .update(cx, |_, window, _| window.reset_input_latency_measurements())
    })
    .expect("the native consumer must reset its input measurement window");

    consumer.input(&mut app);
    let mut source = NativeProfilerSource {
        pointer_inputs: 1,
        key_inputs: 1,
        redraws: 0,
    };
    let (next_generation, untraced_paint) =
        consumer.wait_for_submission(&mut app, source, generation);
    generation = next_generation;
    let (untraced_input, untraced_frame) = consumer.snapshot(&mut app);
    assert_native_profiler_outcomes(&untraced_input);
    assert_eq!(untraced_input.latency_histogram.len(), 1);
    assert_eq!(untraced_input.events_per_frame_histogram.len(), 1);
    assert!(untraced_paint.pending_inputs >= 3);
    assert_eq!(
        untraced_input.events_per_frame_histogram.max(),
        untraced_paint.pending_inputs
    );
    assert!(
        untraced_frame.draw_duration_histogram.len() > initial_frame.draw_duration_histogram.len()
    );
    assert!(!untraced_frame.dirty_to_present_histogram.is_empty());
    assert!(untraced_input.refresh_request_count >= 4);
    assert!(untraced_input.coalesced_refresh_request_count >= 2);
    assert!(untraced_input.refresh_callsites.iter().any(|callsite| {
        callsite.file == file!()
            && callsite.request_count >= 2
            && callsite.coalesced_request_count > 0
    }));
    assert!(collector.collect_unseen().is_empty());
    app.update_for_test(|cx| {
        consumer
            .handle
            .update(cx, |_, window, _| window.reset_input_latency_measurements())
    })
    .expect("the trace phase must start a fresh input measurement window");
    let (reset_input, reset_frame) = consumer.snapshot(&mut app);
    assert!(reset_input.latency_histogram.is_empty());
    assert_eq!(reset_input.drawn_frame_count, 0);
    assert_eq!(reset_input.present_attempt_count, 0);
    assert_eq!(reset_input.refresh_request_count, 0);
    assert!(reset_input.refresh_callsites.is_empty());
    assert_eq!(
        reset_frame.draw_duration_histogram.len(),
        untraced_frame.draw_duration_histogram.len()
    );
    assert_eq!(
        reset_frame.dirty_to_present_histogram.len(),
        untraced_frame.dirty_to_present_histogram.len()
    );

    assert!(profiler::set_trace_enabled(true));
    assert!(profiler::trace_enabled());
    let (input_start, input_end) = consumer.input(&mut app);
    source.pointer_inputs += 1;
    source.key_inputs += 1;
    let (traced_generation, paint) = consumer.wait_for_submission(&mut app, source, generation);
    let (traced_input, traced_frame) = consumer.snapshot(&mut app);
    assert_native_profiler_outcomes(&traced_input);
    assert_eq!(traced_input.latency_histogram.len(), 1);
    assert!(paint.pending_inputs >= 3);
    assert_eq!(
        traced_input.events_per_frame_histogram.max(),
        paint.pending_inputs
    );
    let presented = assert_native_profiler_trace(
        &consumer,
        &mut collector,
        traced_generation,
        &paint,
        &traced_frame,
    );
    assert_native_foreground_journal(
        &consumer,
        &mut journal_collector,
        &mut independent_collector,
        presented,
        &paint,
    );
    assert!(input_start <= input_end && input_end <= paint.painted_at);
    let minimum_latency = u64::try_from(presented.present_end.duration_since(input_end).as_nanos())
        .expect("bounded native input latency lower bound");
    let maximum_latency =
        u64::try_from(presented.present_end.duration_since(input_start).as_nanos())
            .expect("bounded native input latency upper bound");
    assert_eq!(
        traced_input
            .latency_histogram
            .count_between(minimum_latency, maximum_latency),
        1
    );

    consumer.settle(&mut app);
    let (settled_input, _) = consumer.snapshot(&mut app);
    let followup_events = collector.collect_unseen();
    assert!(followup_events.iter().all(|event| match event {
        FrameEvent::Draw(timing) => timing.window_id == consumer.handle.window_id(),
        FrameEvent::Present(timing) => timing.window_id == consumer.handle.window_id(),
    }));
    source.redraws += 1;
    app.update_for_test(|cx| {
        consumer.handle.update(cx, |root, window, cx| {
            root.source.redraws += 1;
            window.refresh();
            cx.notify();
        })
    })
    .expect("the native profiler consumer must redraw without input");
    let (redraw_generation, paint) =
        consumer.wait_for_submission(&mut app, source, traced_generation);
    let (redraw_input, redraw_frame) = consumer.snapshot(&mut app);
    assert_native_profiler_outcomes(&redraw_input);
    assert_eq!(
        redraw_input.latency_histogram.len(),
        settled_input.latency_histogram.len()
    );
    assert_eq!(
        redraw_input.events_per_frame_histogram.len(),
        settled_input.events_per_frame_histogram.len()
    );
    assert!(
        redraw_frame.draw_duration_histogram.len() > traced_frame.draw_duration_histogram.len()
    );
    assert_native_profiler_trace(
        &consumer,
        &mut collector,
        redraw_generation,
        &paint,
        &redraw_frame,
    );
    let retired_id = consumer.handle.window_id();
    consumer.retire(&mut app, &platform, &observation);
    assert!(collector.collect_unseen().is_empty());

    assert!(profiler::set_trace_enabled(false));
    let rebuilt = NativeProfilerWindow::open(&mut app, &platform);
    assert_ne!(rebuilt.handle.window_id(), retired_id);
    let (initial_rebuilt_generation, _) =
        rebuilt.wait_for_submission(&mut app, NativeProfilerSource::default(), 0);
    rebuilt.settle(&mut app);
    assert!(collector.collect_unseen().is_empty());
    let (initial_rebuilt_input, _) = rebuilt.snapshot(&mut app);
    assert_native_profiler_outcomes(&initial_rebuilt_input);
    assert!(profiler::set_trace_enabled(true));
    app.update_for_test(|cx| {
        rebuilt.handle.update(cx, |root, window, cx| {
            root.source.redraws += 1;
            window.refresh();
            cx.notify();
        })
    })
    .expect("the rebuilt visible window must expose a complete dirty-to-present trace");
    let (rebuilt_generation, rebuilt_paint) = rebuilt.wait_for_submission(
        &mut app,
        NativeProfilerSource {
            redraws: 1,
            ..NativeProfilerSource::default()
        },
        initial_rebuilt_generation,
    );
    let (rebuilt_input, rebuilt_frame) = rebuilt.snapshot(&mut app);
    assert_native_profiler_outcomes(&rebuilt_input);
    assert!(rebuilt_input.latency_histogram.is_empty());
    assert!(rebuilt_input.events_per_frame_histogram.is_empty());
    assert_native_profiler_trace(
        &rebuilt,
        &mut collector,
        rebuilt_generation,
        &rebuilt_paint,
        &rebuilt_frame,
    );
    assert!(profiler::set_trace_enabled(false));
    assert!(!profiler::trace_enabled());
    assert!(collector.collect_unseen().is_empty());
    app.update_for_test(|cx| {
        rebuilt.handle.update(cx, |root, window, cx| {
            root.source.redraws += 1;
            window.refresh();
            cx.notify();
        })
    })
    .expect("the rebuilt native window must continue aggregating after trace disable");
    rebuilt.wait_for_submission(
        &mut app,
        NativeProfilerSource {
            redraws: 2,
            ..NativeProfilerSource::default()
        },
        rebuilt_generation,
    );
    let (disabled_input, disabled_frame) = rebuilt.snapshot(&mut app);
    assert_native_profiler_outcomes(&disabled_input);
    assert!(disabled_input.latency_histogram.is_empty());
    assert!(
        disabled_frame.draw_duration_histogram.len() > rebuilt_frame.draw_duration_histogram.len()
    );
    assert!(
        disabled_frame.dirty_to_present_histogram.len()
            > rebuilt_frame.dirty_to_present_histogram.len()
    );
    assert!(collector.collect_unseen().is_empty());
    rebuilt.retire(&mut app, &platform, &observation);

    app.update_for_test(|cx| cx.shutdown());
    assert!(monitor_completed.load(std::sync::atomic::Ordering::Acquire));
    let (reason, monitor_thread, first_present_at) = monitor_receiver
        .try_recv()
        .expect("App shutdown must await the monitor's final flush");
    assert_eq!(reason, HangMonitorPollReason::Flush);
    assert_ne!(monitor_thread, foreground_thread);
    assert!(first_present_at.is_some());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native profiler platform message HWND retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() } && callback_weak.upgrade().is_none(),
    );
    assert!(platform_lifetime.upgrade().is_none());
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}

fn assert_native_foreground_journal(
    consumer: &NativeProfilerWindow,
    collector: &mut ForegroundJournalCollector,
    independent: &mut ForegroundJournalCollector,
    presented: open_gpui::PresentTiming,
    paint: &NativeProfilerPaint,
) {
    let drained = collector.collect_unseen();
    let second = independent.collect_unseen();
    assert_eq!(drained.lost, 0);
    assert_eq!(second.lost, 0);
    let markers = |entries: &[ForegroundJournalEntry]| {
        entries
            .iter()
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Input(input)) => {
                    Some((input.kind, None, input.start))
                }
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(frame)) => Some((
                    "presented",
                    Some(frame.frame.window_id),
                    frame.presentation.present_start,
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let observed = markers(&drained.entries);
    assert_eq!(observed, markers(&second.entries));
    let platform_signals = |entries: &[ForegroundJournalEntry]| {
        entries
            .iter()
            .filter_map(|entry| {
                let ForegroundJournalEntry::PlatformSignal(signal) = entry else {
                    return None;
                };
                Some((
                    signal.window_id,
                    signal.signal_at,
                    signal.handled_at,
                    signal.source,
                ))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        platform_signals(&drained.entries),
        platform_signals(&second.entries)
    );
    assert!(observed.iter().any(|(kind, _, _)| *kind == "mouse_down"));
    assert!(observed.iter().any(|(kind, _, _)| *kind == "key_down"));
    assert!(drained.entries.iter().any(|entry| {
        let ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(frame)) = entry else {
            return false;
        };
        frame.frame.window_id == consumer.handle.window_id()
            && frame.presentation.present_start == presented.present_start
            && frame.presentation.present_end == presented.present_end
            && frame.frame.draw_start <= paint.painted_at
            && paint.painted_at <= frame.frame.draw_end
    }));
    let callbacks = consumer.signals.borrow();
    let matching_signals = drained
        .entries
        .iter()
        .filter_map(|entry| {
            let ForegroundJournalEntry::PlatformSignal(signal) = entry else {
                return None;
            };
            let at = signal.signal_at?;
            let callback = callbacks.iter().find(|callback| {
                signal.window_id == consumer.handle.window_id()
                    && callback.options.signal_at == Some(at)
                    && callback.options.signal_source == signal.source
            })?;
            assert!(at <= callback.entered_at);
            assert!(callback.entered_at <= signal.handled_at);
            Some(signal)
        })
        .count();
    assert!(
        matching_signals > 0,
        "native callbacks must reach the raw platform-signal journal"
    );
    assert!(collector.collect_unseen().entries.is_empty());
    assert!(independent.collect_unseen().entries.is_empty());
}

fn wait_for_native_platform_signal(
    consumer: &NativeProfilerWindow,
    collector: &mut ForegroundJournalCollector,
    at: Option<Instant>,
    source: FrameRequestSource,
) -> PlatformSignal {
    let mut observed = None;
    pump_messages_until(
        "native callback timestamp/source in the raw journal",
        || {
            let drained = collector.collect_unseen();
            assert_eq!(drained.lost, 0);
            for entry in drained.entries {
                if let ForegroundJournalEntry::PlatformSignal(signal) = entry
                    && signal.window_id == consumer.handle.window_id()
                    && signal.signal_at == at
                    && signal.source == source
                {
                    assert!(
                        observed.replace(signal).is_none(),
                        "coalesced callbacks must emit one raw signal"
                    );
                }
            }
            observed.is_some()
        },
    );
    let signal = observed.expect("the exact native callback signal must reach the raw journal");
    let callbacks = consumer.signals.borrow();
    let callback = callbacks
        .iter()
        .find(|callback| {
            callback.options.signal_at == at && callback.options.signal_source == source
        })
        .expect("the raw journal must retain the pair observed at the production callback");
    if let Some(at) = at {
        assert!(at <= callback.entered_at);
    }
    assert!(callback.entered_at <= signal.handled_at);
    signal
}

/// Exercises injected timestamp/source pairs through real callbacks and submissions, then observes
/// an actual background VSync signal before native delivery. Visibility and retirement use real
/// HWND operations; the reused-HWND control substitutes a stale registration, not a graphics receipt.
#[test]
fn native_frame_signals_preserve_coalescing_visibility_retirement_and_vsync_origin() {
    discard_stale_quit_messages();
    let _restore_trace = RestoreNativeProfilerTrace(profiler::trace_enabled());
    profiler::set_trace_enabled(false);
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(WindowsPlatform::new(false).expect("native frame-signal backend"));
    let platform_hwnd = platform.handle;
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut collector = app.update_for_test(|cx| cx.foreground_journal().collector());
    let consumer = NativeProfilerWindow::open(&mut app, &platform);
    let mut source = NativeProfilerSource::default();
    let (mut generation, _) = consumer.wait_for_submission(&mut app, source, 0);
    consumer.settle(&mut app);
    collector.collect_unseen();
    consumer.native.state.frame_signal.take();
    unsafe {
        SendMessageW(consumer.registration.as_raw(), WM_PAINT, None, None);
    }
    wait_for_native_platform_signal(
        &consumer,
        &mut collector,
        None,
        FrameRequestSource::NativeCallback,
    );

    for first_source in [
        FrameRequestSource::LocalSchedule,
        FrameRequestSource::NativeCallback,
    ] {
        let later_source = match first_source {
            FrameRequestSource::LocalSchedule => FrameRequestSource::NativeCallback,
            FrameRequestSource::NativeCallback => FrameRequestSource::LocalSchedule,
        };
        consumer.native.state.frame_signal.take();
        consumer.signals.borrow_mut().clear();
        let first_at = Instant::now();
        let later_at = first_at + Duration::from_nanos(1);
        platform.raw_window_handles.record_frame_signal(
            consumer.registration,
            Some(later_at),
            later_source,
        );
        platform.raw_window_handles.record_frame_signal(
            consumer.registration,
            Some(first_at),
            first_source,
        );
        source.redraws += 1;
        app.update_for_test(|cx| {
            consumer
                .handle
                .update(cx, |root, window, cx| {
                    root.source = source;
                    window.refresh();
                    cx.notify();
                })
                .expect("coalesced native frames must retain their real consumer");
            unsafe {
                SendMessageW(consumer.registration.as_raw(), WM_PAINT, None, None);
                SendMessageW(
                    consumer.registration.as_raw(),
                    crate::WM_GPUI_FORCE_UPDATE_WINDOW,
                    Some(WPARAM(consumer.registration.generation())),
                    None,
                );
            }
            consumer.native.request_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: false,
                signal_at: None,
                signal_source: FrameRequestSource::LocalSchedule,
            });
        });
        wait_for_native_platform_signal(&consumer, &mut collector, Some(first_at), first_source);
        let callbacks = consumer.signals.borrow();
        assert!(
            callbacks
                .iter()
                .any(|callback| callback.options.force_render)
        );
        assert!(callbacks.iter().any(|callback| {
            callback.options.require_presentation
                && callback.options.signal_source == FrameRequestSource::LocalSchedule
                && callback
                    .options
                    .signal_at
                    .is_some_and(|at| first_at <= at && at <= callback.entered_at)
        }));
        drop(callbacks);
        generation = consumer.wait_for_submission(&mut app, source, generation).0;
        consumer.settle(&mut app);
        collector.collect_unseen();
    }

    for (hide, show) in [
        (SW_HIDE, SW_SHOWNOACTIVATE),
        (SW_SHOWMINNOACTIVE, SW_RESTORE),
    ] {
        let stale_at = Instant::now();
        platform.raw_window_handles.record_frame_signal(
            consumer.registration,
            Some(stale_at),
            FrameRequestSource::NativeCallback,
        );
        let _ = unsafe { ShowWindow(consumer.registration.as_raw(), hide) };
        assert!(consumer.native.state.frame_signal.take().is_none());
        platform.raw_window_handles.record_frame_signal(
            consumer.registration,
            Some(stale_at),
            FrameRequestSource::LocalSchedule,
        );
        assert!(consumer.native.state.frame_signal.take().is_none());
        consumer.signals.borrow_mut().clear();
        source.redraws += 1;
        app.update_for_test(|cx| {
            consumer
                .handle
                .update(cx, |root, window, cx| {
                    root.source = source;
                    window.refresh();
                    cx.notify();
                })
                .expect("hidden and minimized consumers remain logically live");
        });
        let _ = unsafe { ShowWindow(consumer.registration.as_raw(), show) };
        generation = consumer.wait_for_submission(&mut app, source, generation).0;
        consumer.settle(&mut app);
        assert!(
            consumer
                .signals
                .borrow()
                .iter()
                .all(|callback| callback.options.signal_at != Some(stale_at))
        );
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(drained.entries.iter().all(|entry| {
            !matches!(entry, ForegroundJournalEntry::PlatformSignal(signal) if signal.signal_at == Some(stale_at))
        }));
    }

    let retired_registration = consumer.registration;
    let retired_signal = consumer.native.state.frame_signal.clone();
    platform.raw_window_handles.record_frame_signal(
        retired_registration,
        Some(Instant::now()),
        FrameRequestSource::NativeCallback,
    );
    consumer.retire(&mut app, &platform, &observation);
    assert!(retired_signal.take().is_none());
    assert_eq!(
        Arc::strong_count(&retired_signal),
        1,
        "retired registrations must release their signal association"
    );
    platform.raw_window_handles.record_frame_signal(
        retired_registration,
        Some(Instant::now()),
        FrameRequestSource::LocalSchedule,
    );
    assert!(retired_signal.take().is_none());

    let rebuilt = NativeProfilerWindow::open(&mut app, &platform);
    assert_ne!(
        rebuilt.registration.generation(),
        retired_registration.generation()
    );
    assert!(!Arc::ptr_eq(
        &retired_signal,
        &rebuilt.native.state.frame_signal
    ));
    rebuilt.wait_for_submission(&mut app, NativeProfilerSource::default(), 0);
    rebuilt.settle(&mut app);
    rebuilt.native.state.frame_signal.take();
    let reused_hwnd_snapshot = RegisteredWindow::new(
        rebuilt.registration.as_raw(),
        retired_registration.generation(),
        retired_registration.window_id(),
    );
    platform.raw_window_handles.record_frame_signal(
        reused_hwnd_snapshot,
        Some(Instant::now()),
        FrameRequestSource::NativeCallback,
    );
    assert!(rebuilt.native.state.frame_signal.take().is_none());
    collector.collect_unseen();

    platform.begin_vsync_thread();
    let captured = (0..MAX_MESSAGE_PUMP_ATTEMPTS)
        .find_map(|_| {
            if let Some(signal) = rebuilt.native.state.frame_signal.take() {
                rebuilt.native.state.frame_signal.record(signal.0, signal.1);
                Some(signal)
            } else {
                std::thread::sleep(Duration::from_millis(5));
                None
            }
        })
        .expect("the actual background VSync producer must capture before foreground delivery");
    unsafe {
        SendMessageW(rebuilt.registration.as_raw(), WM_PAINT, None, None);
    }
    wait_for_native_platform_signal(&rebuilt, &mut collector, Some(captured.0), captured.1);
    rebuilt.retire(&mut app, &platform, &observation);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "frame-signal platform retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
}
