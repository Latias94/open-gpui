//! Official generic animation controls consumed by an actual Windows Application and Render.
//!
//! Capped, synchronized animations run on the production executor clock. Source paint observations,
//! nonempty accepted/Submitted generations and exact-generation GPU captures qualify visible
//! progression, reduced-motion endpoints, restoration and retirement while a delayed frame is pending.
//! Synthetic input enters the existing native callback; this does not qualify OS input injection,
//! physical foreground focus, frame-rate budgets, compositor scanout or display latency.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use open_gpui::{
    Animation, AnimationExt, AnyWindowHandle, FocusHandle, InteractiveElement, KeyDownEvent,
    Keystroke, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, WeakEntity, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, black, fill, rgb,
};

const ANIMATION_DURATION: Duration = Duration::from_millis(1200);
const PENDING_FRAME_INTERVAL: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativeAnimationSource {
    epoch: u32,
    pointer_inputs: u32,
    key_inputs: u32,
    reduced_motion: bool,
    long_delay: bool,
}

#[derive(Clone, Copy, Debug)]
struct NativeAnimationPaint {
    window_id: WindowId,
    previous_revision: u64,
    source: NativeAnimationSource,
    phase: f32,
    bounds: Bounds<Pixels>,
}

struct NativeAnimationRoot {
    focus: FocusHandle,
    source: NativeAnimationSource,
    paints: [Rc<Cell<Option<NativeAnimationPaint>>>; 3],
}

fn animation_color(index: usize, phase: f32) -> u32 {
    let step = (phase * 192.).round() as u32;
    ((32 + step) << 16) | ((64 + index as u32 * 48) << 8) | (224 - step)
}

fn animation_source_color(source: NativeAnimationSource) -> u32 {
    0x204060
        + source.epoch * 0x400000
        + source.pointer_inputs * 0x200000
        + source.key_inputs * 0x2000
        + u32::from(source.reduced_motion) * 0x20
}

fn animation_swatch(
    index: usize,
    source: NativeAnimationSource,
    paint: Rc<Cell<Option<NativeAnimationPaint>>>,
) -> impl IntoElement {
    let animation = Animation::new(ANIMATION_DURATION);
    let animation = if index < 2 {
        animation.repeat_synced()
    } else {
        animation
    };
    let max_fps = if source.long_delay {
        1. / PENDING_FRAME_INTERVAL.as_secs_f32()
    } else {
        12.
    };
    let phase = Rc::new(Cell::new(0.));
    let painted_phase = phase.clone();
    div()
        .absolute()
        .left(px(16. + index as f32 * 84.))
        .top(px(16.))
        .w(px(64.))
        .h(px(64.))
        .with_animation(
            ("native-generic-animation", source.epoch * 3 + index as u32),
            animation.with_max_fps(max_fps),
            move |element, value| {
                assert!(value.is_finite() && (0.0..=1.0).contains(&value));
                phase.set(value);
                element.bg(rgb(animation_color(index, value)))
            },
        )
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    paint.set(Some(NativeAnimationPaint {
                        window_id: window.window_handle().window_id(),
                        previous_revision: window.rendered_frame_revision(),
                        source,
                        phase: painted_phase.get(),
                        bounds,
                    }));
                },
            )
            .size_full(),
        )
}

impl Render for NativeAnimationRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = NativeAnimationSource {
            reduced_motion: cx.reduce_motion(),
            ..self.source
        };
        div()
            .id("native-animation-consumer")
            .relative()
            .size_full()
            .bg(black())
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|root, _, window, cx| {
                    root.source.pointer_inputs += 1;
                    window.focus(&root.focus, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "a" {
                    root.source.key_inputs += 1;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .children(self.paints.iter().enumerate().map(|(index, paint)| {
                animation_swatch(index, source, paint.clone()).into_any_element()
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(animation_source_color(source))));
                    },
                )
                .absolute()
                .left(px(16.))
                .top(px(112.))
                .w(px(232.))
                .h(px(32.)),
            )
    }
}

struct NativeAnimationWindow {
    handle: WindowHandle<NativeAnimationRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    root: WeakEntity<NativeAnimationRoot>,
    paints: [Rc<Cell<Option<NativeAnimationPaint>>>; 3],
}

impl NativeAnimationWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let paints = std::array::from_fn(|_| Rc::new(Cell::new(None)));
        let handle = app
            .update_for_test(|cx| {
                assert!(!cx.is_test());
                assert!(!cx.background_executor().is_test());
                assert!(!cx.to_async().is_test());
                assert!(!cx.reduce_motion());
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(304.), px(200.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let focus = cx.focus_handle();
                            window.focus(&focus, cx);
                            NativeAnimationRoot {
                                focus,
                                source: NativeAnimationSource::default(),
                                paints: paints.clone(),
                            }
                        })
                    },
                )
            })
            .expect("the actual generic-animation consumer window must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("the animation window must have its exact HWND registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("the animation HWND must have a native owner");
        assert_eq!(native.registration, registration);
        let root = app
            .update_for_test(|cx| handle.root(cx))
            .expect("the native animation root must be accessible")
            .downgrade();
        Self {
            handle,
            registration,
            native,
            root,
            paints,
        }
    }

    fn revision(&self, app: &mut Application) -> u64 {
        app.update_for_test(|cx| {
            self.handle
                .update(cx, |_, window, _| window.rendered_frame_revision())
        })
        .expect("the animation window must remain live before its next control")
    }

    fn reduce_motion(&self, app: &mut Application, reduced: bool) -> u64 {
        app.update_for_test(|cx| {
            self.handle.update(cx, |_, window, cx| {
                let revision = window.rendered_frame_revision();
                cx.set_reduce_motion(reduced);
                assert_eq!(cx.reduce_motion(), reduced);
                revision
            })
        })
        .expect("the App preference must update the same native animation window")
    }

    #[track_caller]
    fn submitted(
        &self,
        app: &mut Application,
        source: NativeAnimationSource,
        previous: u64,
        mut wanted: impl FnMut(&[NativeAnimationPaint; 3]) -> bool,
    ) -> NativeAnimationSubmission {
        let mut submitted = None;
        let mut last_state = None;
        let description = format!(
            "native animation exact source and nonempty Submitted generation after {previous}: {source:?}"
        );
        pump_messages_until_with_delay(&description, Duration::from_millis(5), || {
            submitted = app
                .update_for_test(|cx| {
                    self.handle.update(cx, |root, window, _| {
                        let facts = window.presentation_facts();
                        let paints = self.paints.each_ref().map(|paint| paint.get());
                        last_state = Some((facts, window.rendered_frame_revision(), paints));
                        let [Some(first), Some(second), Some(oneshot)] = paints else {
                            return None;
                        };
                        let paints = [first, second, oneshot];
                        let attempt = facts.latest_present_attempt?;
                        let diagnostics = window.submitted_frame_diagnostics_for_test()?;
                        if !(attempt.generation > previous
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && diagnostics.generation() == attempt.generation
                            && paints.iter().all(|paint| {
                                paint.window_id == self.handle.window_id()
                                    && paint.source == source
                                    && paint.previous_revision.checked_add(1)
                                        == Some(attempt.generation)
                                    && paint.bounds.size.width > px(0.)
                                    && paint.bounds.size.height > px(0.)
                            })
                            && wanted(&paints))
                        {
                            return None;
                        }
                        assert!(root.focus.is_focused(window));
                        let image = window
                            .render_to_image()
                            .expect("actual native animated Scene GPU readback");
                        let capture = window.capture_frame();
                        assert!(capture.unsupported_reason.is_none(), "{capture:?}");
                        assert_eq!(capture.metadata.window_id, self.handle.window_id());
                        assert_eq!(
                            capture.metadata.framework_frame_generation,
                            attempt.generation
                        );
                        assert_eq!(capture.metadata.scale_factor, window.scale_factor());
                        assert!(capture.metadata.scale_factor.is_finite());
                        assert!(capture.metadata.scale_factor > 0.);
                        assert_eq!(
                            capture.metadata.logical_viewport_size,
                            window.viewport_size()
                        );
                        assert_eq!(
                            capture.metadata.physical_viewport_size,
                            window
                                .viewport_size()
                                .to_device_pixels(window.scale_factor())
                        );
                        assert_eq!(
                            image.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        assert_eq!(
                            capture.image.as_ref(),
                            Some(&image),
                            "both public GPU capture paths must read the same accepted Scene"
                        );
                        assert_eq!(window.rendered_frame_revision(), attempt.generation);
                        assert_eq!(
                            window.presentation_facts().latest_present_attempt,
                            facts.latest_present_attempt,
                            "GPU readback must preserve the real native presentation observation"
                        );
                        Some(NativeAnimationSubmission {
                            generation: attempt.generation,
                            paints,
                            capture,
                        })
                    })
                })
                .expect("the native animation consumer must remain live through capture");
            submitted.is_some()
        });
        let submitted = submitted.unwrap_or_else(|| {
            panic!("the animation source must reach real Submitted pixels: {last_state:?}")
        });
        submitted.assert_pixels();
        submitted
    }

    fn retire(
        self,
        app: &mut Application,
        platform: &WindowsPlatform,
        observation: &NativeWindowTestObservation,
        pending_since: Instant,
    ) {
        let native_lifetime = Rc::downgrade(&self.native);
        assert!(self.root.upgrade().is_some());
        assert!(
            platform
                .raw_window_handles
                .read()
                .contains(&self.registration)
        );
        app.update_for_test(|cx| {
            self.handle.update(cx, |_, window, cx| {
                assert!(
                    pending_since.elapsed() < PENDING_FRAME_INTERVAL,
                    "logical removal must precede the new animation timer's earliest deadline"
                );
                window.remove_window(cx);
            })
        })
        .expect("the animation window must close through its existing public owner");
        pump_messages_until_with_delay(
            "animation exact HWND, registration and view retirement",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(self.registration.as_raw())).as_bool() }
                    && !platform
                        .raw_window_handles
                        .read()
                        .contains(&self.registration)
                    && platform.inner.native_retirement_is_settled()
                    && self.root.upgrade().is_none()
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
            panic!("unexpected animation consumer retirement sequence: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
        assert!(app.update_for_test(|cx| {
            assert!(self.handle.update(cx, |_, _, _| ()).is_err());
            cx.windows().is_empty()
        }));
        assert!(platform.raw_window_handles.read().is_empty());
        drop(self);
        assert!(native_lifetime.upgrade().is_none());
    }
}

struct NativeAnimationSubmission {
    generation: u64,
    paints: [NativeAnimationPaint; 3],
    capture: WindowFrameCapture,
}

impl NativeAnimationSubmission {
    fn assert_pixels(&self) {
        let image = self.capture.image.as_ref().expect("native animation RGBA");
        let sample = |position: open_gpui::Point<Pixels>| {
            let x = (f32::from(position.x) * self.capture.metadata.scale_factor).floor() as u32;
            let y = (f32::from(position.y) * self.capture.metadata.scale_factor).floor() as u32;
            assert!(x < image.width() && y < image.height());
            image.get_pixel(x, y).0
        };
        let assert_color = |actual: [u8; 4], color: u32| {
            let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "animation generation {} RGBA {actual:?} != {expected:?}",
                self.generation
            );
        };
        for (index, paint) in self.paints.iter().enumerate() {
            assert_color(
                sample(paint.bounds.center()),
                animation_color(index, paint.phase),
            );
        }
        assert_color(
            sample(point(px(100.), px(128.))),
            animation_source_color(self.paints[0].source),
        );
        assert_color(sample(point(px(280.), px(100.))), 0);
    }
}

fn animation_phase_distance(first: f32, second: f32) -> f32 {
    let distance = (first - second).abs();
    distance.min(1. - distance)
}

#[test]
#[ignore = "requires an interactive Windows desktop and a DirectX GPU"]
fn native_generic_animations_progress_reduce_restore_and_retire_pending_frames() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native animation backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut weak_app = app.update_for_test(|cx| cx.to_async());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let consumer = NativeAnimationWindow::open(&mut app, &platform);
    let root_lifetime = consumer.root.clone();
    let paint_lifetimes = consumer.paints.each_ref().map(Rc::downgrade);
    let mut source = NativeAnimationSource::default();
    let first = consumer.submitted(&mut app, source, 0, |_| true);
    let progressed = consumer.submitted(&mut app, source, first.generation, |paints| {
        paints.iter().take(2).enumerate().all(|(index, paint)| {
            animation_phase_distance(first.paints[index].phase, paint.phase) >= 0.12
        })
    });
    assert!(
        animation_phase_distance(progressed.paints[0].phase, progressed.paints[1].phase) < 0.05
    );
    assert_ne!(first.capture.image, progressed.capture.image);

    let previous = consumer.reduce_motion(&mut app, true);
    source.reduced_motion = true;
    let reduced = consumer.submitted(&mut app, source, previous, |paints| {
        paints.map(|paint| paint.phase) == [0., 0., 1.]
    });
    let static_since = Instant::now();
    pump_messages_until_with_delay(
        "reduced-motion native endpoint stability through pending timers",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| {
                consumer.handle.update(cx, |_, _, cx| {
                    assert!(cx.reduce_motion());
                    for (index, paint) in consumer.paints.iter().enumerate() {
                        let paint = paint.get().expect("submitted static animation source");
                        assert_eq!(paint.source, source);
                        assert_eq!(paint.phase, if index == 2 { 1. } else { 0. });
                    }
                })
            })
            .expect("the static native animation window must remain live");
            static_since.elapsed() >= Duration::from_millis(220)
        },
    );

    let previous = consumer.revision(&mut app);
    assert!(
        !consumer
            .native
            .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(100.), px(128.)),
                click_count: 1,
                ..Default::default()
            }))
            .propagate
    );
    consumer
        .native
        .dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(100.), px(128.)),
            click_count: 1,
            ..Default::default()
        }));
    source.pointer_inputs += 1;
    let clicked = consumer.submitted(&mut app, source, previous, |_| true);
    assert_eq!(clicked.paints.map(|paint| paint.phase), [0., 0., 1.]);
    assert_ne!(reduced.capture.image, clicked.capture.image);
    let previous = consumer.revision(&mut app);
    assert!(
        !consumer
            .native
            .dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("a")
                    .expect("native animation key")
                    .with_simulated_ime(),
                is_held: false,
                prefer_character_input: false,
            }))
            .propagate
    );
    source.key_inputs += 1;
    let keyed = consumer.submitted(&mut app, source, previous, |_| true);
    assert_eq!(keyed.paints.map(|paint| paint.phase), [0., 0., 1.]);
    assert_ne!(clicked.capture.image, keyed.capture.image);

    let previous = consumer.reduce_motion(&mut app, false);
    source.reduced_motion = false;
    let restored = consumer.submitted(&mut app, source, previous, |paints| {
        paints
            .iter()
            .take(2)
            .all(|paint| paint.phase > 0.1 && paint.phase < 0.9)
    });
    let resumed = consumer.submitted(&mut app, source, restored.generation, |paints| {
        paints.iter().take(2).enumerate().all(|(index, paint)| {
            animation_phase_distance(restored.paints[index].phase, paint.phase) >= 0.12
        })
    });
    assert_ne!(keyed.capture.image, restored.capture.image);
    assert_ne!(restored.capture.image, resumed.capture.image);

    let (previous, pending_since) = app
        .update_for_test(|cx| {
            consumer.handle.update(cx, |root, window, cx| {
                let previous = window.rendered_frame_revision();
                let pending_since = Instant::now();
                root.source.epoch += 1;
                root.source.long_delay = true;
                cx.notify();
                (previous, pending_since)
            })
        })
        .expect("the long-delay animation must be mounted on the same native window");
    source.epoch += 1;
    source.long_delay = true;
    let pending = consumer.submitted(&mut app, source, previous, |_| true);
    assert!(pending.generation > previous);
    let timer_barrier = Rc::new(Cell::new(None));
    app.update_for_test(|cx| {
        let timer_barrier = timer_barrier.clone();
        cx.foreground_executor()
            .spawn(async move {
                timer_barrier.set(Some(Instant::now()));
            })
            .detach();
    });
    pump_messages_until_with_delay(
        "FIFO foreground barrier after native animation timers start",
        Duration::from_millis(5),
        || timer_barrier.get().is_some(),
    );
    let timer_deadline_upper_bound = timer_barrier
        .get()
        .expect("the foreground barrier must run after earlier animation tasks")
        + PENDING_FRAME_INTERVAL;
    assert!(
        pending_since.elapsed() < PENDING_FRAME_INTERVAL,
        "close must begin before the first newly mounted four-second animation timer can fire"
    );
    let handle = consumer.handle;
    consumer.retire(&mut app, &platform, &observation, pending_since);
    assert!(root_lifetime.upgrade().is_none());
    assert!(
        paint_lifetimes
            .iter()
            .all(|paint| paint.upgrade().is_none())
    );
    pump_messages_until_with_delay(
        "retired animation delayed-frame deadline passes without resurrecting its window",
        Duration::from_millis(10),
        || {
            assert!(app.update_for_test(|cx| {
                assert!(handle.update(cx, |_, _, _| ()).is_err());
                cx.windows().is_empty()
            }));
            assert!(root_lifetime.upgrade().is_none());
            assert!(platform.raw_window_handles.read().is_empty());
            Instant::now() >= timer_deadline_upper_bound + Duration::from_millis(120)
        },
    );
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let released = weak_app
        .update_window(AnyWindowHandle::from(handle), |_, _, _| ())
        .expect_err("delayed animations must not retain the App behind its weak async context");
    assert_eq!(released.to_string(), "app was released");
    drop(weak_app);
    drop(platform);
    pump_messages_until_with_delay(
        "native animation platform HWND, App and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
