use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    AnyView, Entity, StyleRefinement, WindowHandle, WindowInitialPresentationStatus, blue,
    deferred, fill, red,
};

struct NativeCacheModel(usize);

#[derive(Default)]
struct NativeCacheCounters {
    root_renders: Cell<usize>,
    child_renders: Cell<usize>,
    canvas_paints: Cell<usize>,
    painted_value: Cell<usize>,
}

impl NativeCacheCounters {
    fn cached_state(&self) -> (usize, usize, usize) {
        (
            self.child_renders.get(),
            self.canvas_paints.get(),
            self.painted_value.get(),
        )
    }
}

struct NativeCacheChild {
    model: Entity<NativeCacheModel>,
    deferred_child: bool,
    counters: Rc<NativeCacheCounters>,
}

impl Render for NativeCacheChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.counters
            .child_renders
            .set(self.counters.child_renders.get() + 1);
        let model = self.model.clone();
        let counters = self.counters.clone();
        let child = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let value = model.read(cx).0;
                window.paint_quad(fill(bounds, if value == 1 { red() } else { blue() }));
                counters.painted_value.set(value);
                counters.canvas_paints.set(counters.canvas_paints.get() + 1);
            },
        )
        .size(px(100.0));
        if self.deferred_child {
            deferred(child).into_any_element()
        } else {
            child.into_any_element()
        }
    }
}

struct NativeCacheRoot {
    child: Entity<NativeCacheChild>,
    deferred_parent: bool,
    counters: Rc<NativeCacheCounters>,
}

impl Render for NativeCacheRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.counters
            .root_renders
            .set(self.counters.root_renders.get() + 1);
        let child =
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size(px(100.0)));
        if self.deferred_parent {
            deferred(child).into_any_element()
        } else {
            child.into_any_element()
        }
    }
}

fn wait_for_native_cache_submission(
    app: &mut Application,
    window: WindowHandle<NativeCacheRoot>,
    previous_generation: u64,
    description: &str,
) -> u64 {
    let mut submitted_generation = None;
    pump_messages_until_with_delay(description, Duration::from_millis(5), || {
        let (facts, rendered_generation, diagnostics_generation) = app
            .update_for_test(|cx| {
                window.update(cx, |_, window, _| {
                    (
                        window.presentation_facts(),
                        window.rendered_frame_revision(),
                        window
                            .submitted_frame_diagnostics_for_test()
                            .map(|diagnostics| diagnostics.generation()),
                    )
                })
            })
            .expect("the native cache window must remain open while awaiting submission");
        let Some(attempt) = facts.latest_present_attempt else {
            return false;
        };
        if facts.initial_presentation == WindowInitialPresentationStatus::Completed
            && facts.native_visible
            && attempt.generation > previous_generation
            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
            && attempt.contained_valid_primitives
            && facts.frame_accepted_generation == Some(attempt.generation)
            && facts.present_submitted_generation == Some(attempt.generation)
            && facts.non_empty_presented_generation == Some(attempt.generation)
            && rendered_generation == attempt.generation
            && diagnostics_generation == Some(attempt.generation)
        {
            submitted_generation = Some(attempt.generation);
            true
        } else {
            false
        }
    });
    submitted_generation.expect("the native cache frame must be accepted and genuinely submitted")
}

#[test]
fn native_cached_deferred_canvas_keeps_model_subscription_after_replay() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    platform.begin_vsync_thread();

    for deferred_parent in [false, true] {
        for deferred_child in [false, true] {
            let case =
                format!("deferred parent {deferred_parent}, deferred child {deferred_child}");
            let observation_cursor = observation.cursor();
            let counters = Rc::new(NativeCacheCounters::default());
            let (model, child, window) = app.update_for_test(|cx| {
                let model = cx.new(|_| NativeCacheModel(1));
                let child = cx.new(|_| NativeCacheChild {
                    model: model.clone(),
                    deferred_child,
                    counters: counters.clone(),
                });
                let window = cx
                    .open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::centered(
                                size(px(240.0), px(160.0)),
                                cx,
                            )),
                            focus_on_appearing: false,
                            activation_policy: WindowActivationPolicy {
                                accepts_activation: false,
                                focus_on_click: false,
                            },
                            accepts_pointer_input: false,
                            show: true,
                            ..WindowOptions::default()
                        },
                        |_, cx| {
                            cx.new(|_| NativeCacheRoot {
                                child: child.clone(),
                                deferred_parent,
                                counters: counters.clone(),
                            })
                        },
                    )
                    .expect("the native cached canvas window should open");
                (model, child, window)
            });
            let registration = platform
                .raw_window_handles
                .read()
                .iter()
                .copied()
                .find(|registration| registration.window_id() == window.window_id())
                .expect("the cached canvas must own its exact registered HWND");
            let hwnd = registration.as_raw();
            let native_window = platform
                .window_from_hwnd(hwnd)
                .expect("the cached canvas HWND must remain registered");
            assert_eq!(native_window.registration, registration, "{case}");
            let native_facts = native_window
                .observed_platform_facts_for_test()
                .expect("native cache prerequisites must be observable from the HWND");
            assert!(!native_facts.accepts_pointer_input, "{case}");
            assert!(!native_facts.accepts_activation, "{case}");
            assert!(!native_facts.focus_on_click, "{case}");
            let frame_requests = observe_frame_requests(&native_window);
            let initial_paint_cursor = observation.cursor();
            assert!(
                unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() },
                "{case}: the native surface must accept its initialization paint"
            );
            pump_messages_until_with_delay(
                &format!("{case}: native initialization WM_PAINT"),
                Duration::from_millis(5),
                || {
                    observation.any_after(initial_paint_cursor, |event| {
                        event.window().window_id() == registration.window_id()
                            && event.window().native_generation() == registration.generation()
                            && matches!(
                                event.kind(),
                                NativeWindowTestEventKind::WindowMessage {
                                    message: NativeWindowTestMessage::Other(WM_PAINT),
                                    disposition: NativeWindowTestMessageDisposition::Returned(0),
                                    ..
                                }
                            )
                    })
                },
            );
            pump_messages_until_idle(&format!("{case}: native initialization paint follow-up"));
            let mut generation = wait_for_native_cache_submission(
                &mut app,
                window,
                0,
                &format!("{case}: initial native cache submission"),
            );
            pump_messages_until_idle(&format!("{case}: shown native cache follow-up"));
            let before_initialization = app
                .update_for_test(|cx| {
                    window.update(cx, |_, window, _| {
                        let generation = window.rendered_frame_revision();
                        window.refresh();
                        generation
                    })
                })
                .expect("the native cache initialization must own its explicit refresh");
            generation = wait_for_native_cache_submission(
                &mut app,
                window,
                generation.max(before_initialization),
                &format!("{case}: fully initialized native cache submission"),
            );
            assert!(counters.root_renders.get() > 0, "{case}");
            assert!(counters.child_renders.get() > 0, "{case}");
            assert!(counters.canvas_paints.get() > 0, "{case}");
            assert_eq!(counters.painted_value.get(), 1, "{case}");
            let initial_cached_state = counters.cached_state();

            for replay in 1..=2 {
                let root_renders_before = counters.root_renders.get();
                let frame_requests_before = frame_requests.get();
                app.update_for_test(|cx| window.update(cx, |_, _, cx| cx.notify()))
                    .expect("notifying the native root should replay its cached child");
                generation = wait_for_native_cache_submission(
                    &mut app,
                    window,
                    generation,
                    &format!("{case}: native cache replay {replay}"),
                );
                assert!(counters.root_renders.get() > root_renders_before, "{case}");
                assert!(frame_requests.get() > frame_requests_before, "{case}");
                assert_eq!(
                    counters.cached_state(),
                    initial_cached_state,
                    "{case}: replay {replay} must reuse child render and canvas paint"
                );
            }

            let root_renders_before = counters.root_renders.get();
            let frame_requests_before = frame_requests.get();
            app.update_for_test(|cx| {
                model.update(cx, |model, cx| {
                    model.0 = 2;
                    cx.notify();
                });
            });
            generation = wait_for_native_cache_submission(
                &mut app,
                window,
                generation,
                &format!("{case}: native model notification after cache replay"),
            );
            assert!(
                counters.root_renders.get() > root_renders_before,
                "{case}: a paint-only model dependency must still redraw the native root after two reuses"
            );
            assert!(frame_requests.get() > frame_requests_before, "{case}");
            assert_eq!(
                counters.cached_state(),
                initial_cached_state,
                "{case}: non-view model notification redraws the window but does not invalidate the cached child or repaint value 2"
            );

            let root_renders_before = counters.root_renders.get();
            let frame_requests_before = frame_requests.get();
            app.update_for_test(|cx| child.update(cx, |_, cx| cx.notify()));
            wait_for_native_cache_submission(
                &mut app,
                window,
                generation,
                &format!("{case}: explicitly invalidated native cached child"),
            );
            assert!(counters.root_renders.get() > root_renders_before, "{case}");
            assert!(frame_requests.get() > frame_requests_before, "{case}");
            assert!(
                counters.child_renders.get() > initial_cached_state.0,
                "{case}"
            );
            assert!(
                counters.canvas_paints.get() > initial_cached_state.1,
                "{case}"
            );
            assert_eq!(counters.painted_value.get(), 2, "{case}");

            assert!(
                platform.raw_window_handles.read().contains(&registration),
                "{case}: teardown must close the original native registration"
            );
            app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
                .expect("the exact native cached canvas window should close");
            pump_messages_until_with_delay(
                &format!("{case}: exact native cache window retirement"),
                Duration::from_millis(5),
                || {
                    !unsafe { IsWindow(Some(hwnd)).as_bool() }
                        && !is_registered(&platform, hwnd)
                        && platform.inner.native_retirement_is_settled()
                },
            );
            let lifecycle = observation
                .events_after(observation_cursor)
                .into_iter()
                .filter(|event| {
                    event.window().window_id() == registration.window_id()
                        && event.window().native_generation() == registration.generation()
                })
                .filter_map(|event| match event.kind() {
                    NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                        assert_ne!(
                            disposition,
                            NativeWindowTestMessageDisposition::Panicked,
                            "{case}: the native boundary must not swallow an invariant panic"
                        );
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
                panic!("{case}: unexpected native cache retirement sequence: {lifecycle:?}");
            };
            assert_eq!(quiesced, destroyed, "{case}");
            assert_eq!(destroyed, terminal, "{case}");
            assert!(app.update_for_test(|cx| cx.windows().is_empty()), "{case}");
            assert!(platform.raw_window_handles.read().is_empty(), "{case}");
            drop(native_window);
            drop(child);
            drop(model);
        }
    }

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native cache platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
