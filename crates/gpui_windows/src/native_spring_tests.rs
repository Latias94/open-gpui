//! Official Spring playback over a Windows Application, native frame pump and DirectX renderer.
//!
//! Each control has an exact source token, paint observation and nonempty Submitted generation.
//! GPU readback checks both the moving child and source marker. Synthetic input exercises the
//! installed native input callback and logical focus, without requiring physical foreground focus.
//! Executor barriers establish elapsed paused/stopped intervals; they do not qualify frame-rate
//! budgets, compositor scanout, display latency or general spring numerical accuracy.
//! Each transient trajectory allows at most three attempts, each reset using a new element
//! epoch and a captured static initial value. Missing a phase never counts as a successful attempt.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    AnimationExt, FocusHandle, InteractiveElement, KeyDownEvent, Keystroke, MouseButton,
    MouseDownEvent, MouseUpEvent, Pixels, Point, SpringAnimation, SpringConfig, SpringPlayback,
    WeakEntity, WindowFrameCapture, WindowHandle, WindowInitialPresentationStatus, black, fill,
    rgb,
};

const SPRING_TARGET: f32 = 192.;
const SPRING_CONFIG: SpringConfig = SpringConfig::new(0.25, 1., 1.);
const MAX_TRAJECTORY_ATTEMPTS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeSpringSource {
    token: u32,
    epoch: u32,
    pointer_inputs: u32,
    key_inputs: u32,
    target: f32,
    playback: SpringPlayback,
    reduced_motion: bool,
}

impl Default for NativeSpringSource {
    fn default() -> Self {
        Self {
            token: 0,
            epoch: 0,
            pointer_inputs: 0,
            key_inputs: 0,
            target: SPRING_TARGET,
            playback: SpringPlayback::Paused,
            reduced_motion: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeSpringPaint {
    window_id: WindowId,
    previous_revision: u64,
    source: NativeSpringSource,
    value: f32,
    bounds: Bounds<Pixels>,
}

fn spring_value_color(value: f32) -> u32 {
    let step = (value / SPRING_TARGET * 160.).round().clamp(0., 160.) as u32;
    ((32 + step) << 16) | 0x008000 | (224 - step)
}

fn spring_source_color(source: NativeSpringSource) -> u32 {
    let playback = match source.playback {
        SpringPlayback::Running => 0,
        SpringPlayback::Paused => 1,
        SpringPlayback::Stopped => 2,
        SpringPlayback::Completed => 3,
        SpringPlayback::Cancelled => 4,
    };
    let red = 32 + source.token % 64 * 3;
    let green =
        32 + (source.epoch * 3 + source.pointer_inputs * 7 + source.key_inputs * 11) % 64 * 3;
    (red << 16) | (green << 8) | (64 + playback * 24 + u32::from(source.reduced_motion) * 8)
}

struct NativeSpringRoot {
    focus: FocusHandle,
    source: NativeSpringSource,
    paint: Rc<Cell<Option<NativeSpringPaint>>>,
}

impl Render for NativeSpringRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        assert_eq!(cx.reduce_motion(), self.source.reduced_motion);
        let source = self.source;
        let value = Rc::new(Cell::new(0.));
        let painted_value = value.clone();
        let paint = self.paint.clone();
        let animation = SpringAnimation::new(SPRING_CONFIG)
            .to(px(source.target))
            .from(px(0.))
            .playback(source.playback);
        div()
            .id("native-spring-consumer")
            .relative()
            .size_full()
            .bg(black())
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|root, _, window, cx| {
                    root.source.token += 1;
                    root.source.pointer_inputs += 1;
                    root.source.target = 0.;
                    window.focus(&root.focus, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "p" {
                    root.source.token += 1;
                    root.source.key_inputs += 1;
                    root.source.playback = match root.source.playback {
                        SpringPlayback::Paused => SpringPlayback::Running,
                        _ => SpringPlayback::Paused,
                    };
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .absolute()
                    .top(px(16.))
                    .w(px(48.))
                    .with_spring(
                        ("native-position-spring", source.epoch),
                        animation,
                        move |element, output| {
                            let output = f32::from(output);
                            assert!(output.is_finite());
                            value.set(output);
                            element
                                .left(px(16. + output))
                                .bg(rgb(spring_value_color(output)))
                        },
                    )
                    .map_element(|element| element.h(px(64.)))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, (), window, _| {
                                paint.set(Some(NativeSpringPaint {
                                    window_id: window.window_handle().window_id(),
                                    previous_revision: window.rendered_frame_revision(),
                                    source,
                                    value: painted_value.get(),
                                    bounds,
                                }));
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(spring_source_color(source))));
                    },
                )
                .absolute()
                .left(px(16.))
                .top(px(112.))
                .w(px(256.))
                .h(px(32.)),
            )
    }
}

struct NativeSpringWindow {
    handle: WindowHandle<NativeSpringRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    root: WeakEntity<NativeSpringRoot>,
    paint: Rc<Cell<Option<NativeSpringPaint>>>,
}

impl NativeSpringWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let paint = Rc::new(Cell::new(None));
        let handle = app
            .update_for_test(|cx| {
                assert!(!cx.is_test());
                assert!(!cx.background_executor().is_test());
                assert!(!cx.to_async().is_test());
                assert!(!cx.reduce_motion());
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(304.), px(184.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let focus = cx.focus_handle();
                            window.focus(&focus, cx);
                            NativeSpringRoot {
                                focus,
                                source: NativeSpringSource::default(),
                                paint: paint.clone(),
                            }
                        })
                    },
                )
            })
            .expect("the actual Spring consumer window must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("the Spring window must have an exact HWND registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("the Spring HWND must have its native owner");
        assert_eq!(native.registration, registration);
        let root = app
            .update_for_test(|cx| handle.root(cx))
            .expect("the native Spring root must be accessible")
            .downgrade();
        Self {
            handle,
            registration,
            native,
            root,
            paint,
        }
    }

    fn control(
        &self,
        app: &mut Application,
        update: impl FnOnce(&mut NativeSpringSource),
    ) -> (NativeSpringSource, u64) {
        app.update_for_test(|cx| {
            self.handle.update(cx, |root, window, cx| {
                let previous = window.rendered_frame_revision();
                root.source.token += 1;
                update(&mut root.source);
                cx.set_reduce_motion(root.source.reduced_motion);
                cx.notify();
                (root.source, previous)
            })
        })
        .expect("the same Spring source must accept its next control")
    }

    fn source(&self, app: &mut Application) -> (NativeSpringSource, u64) {
        app.update_for_test(|cx| {
            self.handle.update(cx, |root, window, _| {
                (root.source, window.rendered_frame_revision())
            })
        })
        .expect("the Spring source and accepted revision must remain observable")
    }

    fn reset_paused(&self, platform: &WindowsPlatform, app: &mut Application) {
        let (source, previous) = self.control(app, |source| {
            source.epoch += 1;
            source.target = SPRING_TARGET;
            source.playback = SpringPlayback::Paused;
            source.reduced_motion = false;
        });
        let reset = self.submitted(platform, app, source, previous, |_| true);
        assert_eq!(reset.value, 0.);
    }

    fn start_outward(
        &self,
        platform: &WindowsPlatform,
        app: &mut Application,
    ) -> Option<NativeSpringSubmission> {
        let (source, previous) = self.control(app, |source| {
            source.playback = SpringPlayback::Running;
        });
        let outward = self.submitted(platform, app, source, previous, |value| {
            value >= SPRING_TARGET * 0.06
        });
        (outward.value < SPRING_TARGET / 2.).then_some(outward)
    }

    fn carried_then_reversed(
        &self,
        platform: &WindowsPlatform,
        app: &mut Application,
        source: NativeSpringSource,
        start: NativeSpringSubmission,
    ) -> Option<NativeSpringSubmission> {
        let carried = self.submitted(platform, app, source, start.generation, |value| {
            value > start.value + 0.5 || value < start.value - 0.5 || value == 0.
        });
        if carried.value <= start.value + 0.5 {
            return None;
        }
        assert!(carried.value > start.value);
        let mut peak = carried.value;
        let turning = self.submitted(platform, app, source, carried.generation, |value| {
            peak = peak.max(value);
            value < peak - 1. || value == 0.
        });
        if turning.value <= 0. {
            return None;
        }
        let reversed = self.submitted(platform, app, source, turning.generation, |value| {
            value < start.value - 0.5 || value == 0.
        });
        assert!(reversed.value < carried.value);
        (reversed.value > 0.).then_some(reversed)
    }

    #[track_caller]
    fn submitted(
        &self,
        platform: &WindowsPlatform,
        app: &mut Application,
        source: NativeSpringSource,
        previous: u64,
        mut wanted: impl FnMut(f32) -> bool,
    ) -> NativeSpringSubmission {
        let mut result = None;
        let mut last_state = None;
        let description = format!("native Spring Submitted pixels after {previous}: {source:?}");
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            &description,
            Duration::from_millis(8),
            || {
                result = app
                    .update_for_test(|cx| {
                        self.handle.update(cx, |root, window, cx| {
                            let facts = window.presentation_facts();
                            let paint = self.paint.get();
                            last_state = Some((facts, window.rendered_frame_revision(), paint));
                            let paint = paint?;
                            let attempt = facts.latest_present_attempt?;
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
                                && window.submitted_frame_diagnostics_for_test()?.generation()
                                    == attempt.generation
                                && paint.window_id == self.handle.window_id()
                                && paint.source == source
                                && paint.previous_revision.checked_add(1)
                                    == Some(attempt.generation)
                                && paint.bounds.size == size(px(48.), px(64.))
                                && wanted(paint.value))
                            {
                                return None;
                            }
                            assert_eq!(root.source, source);
                            assert_eq!(cx.reduce_motion(), source.reduced_motion);
                            assert!(root.focus.is_focused(window));
                            assert_eq!(self.native.registration, self.registration);
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
                            assert!(
                                (f32::from(paint.bounds.origin.x) - (16. + paint.value)).abs()
                                    <= 1. / capture.metadata.scale_factor
                            );
                            assert_eq!(paint.bounds.origin.y, px(16.));
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
                            let image =
                                capture.image.as_ref().expect("native Spring RGBA readback");
                            assert_eq!(
                                image.dimensions(),
                                (
                                    capture.metadata.physical_viewport_size.width.0 as u32,
                                    capture.metadata.physical_viewport_size.height.0 as u32,
                                )
                            );
                            assert_spring_pixel(
                                &capture,
                                paint.bounds.center(),
                                spring_value_color(paint.value),
                            );
                            assert_spring_pixel(
                                &capture,
                                point(px(100.), px(128.)),
                                spring_source_color(source),
                            );
                            assert_spring_pixel(&capture, point(px(280.), px(96.)), 0);
                            assert_eq!(window.rendered_frame_revision(), attempt.generation);
                            assert_eq!(
                                window.presentation_facts().latest_present_attempt,
                                facts.latest_present_attempt
                            );
                            Some(NativeSpringSubmission {
                                generation: attempt.generation,
                                value: paint.value,
                            })
                        })
                    })
                    .expect("the exact Spring window must remain live until capture");
                result.is_some()
            },
        );
        result.unwrap_or_else(|| {
            panic!("Spring source did not reach Submitted pixels: {last_state:?}")
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeSpringSubmission {
    generation: u64,
    value: f32,
}

fn assert_spring_pixel(capture: &WindowFrameCapture, position: Point<Pixels>, color: u32) {
    let image = capture.image.as_ref().expect("native Spring RGBA readback");
    let x = (f32::from(position.x) * capture.metadata.scale_factor).floor() as u32;
    let y = (f32::from(position.y) * capture.metadata.scale_factor).floor() as u32;
    assert!(x < image.width() && y < image.height());
    let actual = image.get_pixel(x, y).0;
    let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
        "Spring generation {} RGBA {actual:?} != {expected:?}",
        capture.metadata.framework_frame_generation
    );
}

fn spring_executor_barrier(platform: &WindowsPlatform, app: &mut Application) {
    let completed = Rc::new(Cell::new(false));
    app.update_for_test(|cx| {
        let background = cx.background_executor().clone();
        let completed = completed.clone();
        cx.foreground_executor()
            .spawn(async move {
                background.timer(Duration::from_millis(180)).await;
                completed.set(true);
            })
            .detach();
    });
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "production executor elapsed Spring-control barrier",
        Duration::from_millis(5),
        || completed.get(),
    );
}

fn spring_pause_key(consumer: &NativeSpringWindow) {
    assert!(
        !consumer
            .native
            .dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("p")
                    .expect("native Spring pause key")
                    .with_simulated_ime(),
                is_held: false,
                prefer_character_input: false,
            }))
            .propagate
    );
}

fn spring_retarget_trajectory(
    platform: &WindowsPlatform,
    consumer: &NativeSpringWindow,
    app: &mut Application,
) {
    for attempt in 0..MAX_TRAJECTORY_ATTEMPTS {
        if attempt > 0 {
            consumer.reset_paused(platform, app);
        }
        let Some(outward) = consumer.start_outward(platform, app) else {
            continue;
        };
        let (mut source, previous) = consumer.source(app);
        source.token += 1;
        source.pointer_inputs += 1;
        source.target = 0.;
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
        let retargeted = consumer.submitted(platform, app, source, previous, |_| true);
        if retargeted.value <= outward.value || retargeted.value >= SPRING_TARGET / 2. {
            continue;
        }
        if consumer
            .carried_then_reversed(platform, app, source, retargeted)
            .is_some()
        {
            return;
        }
    }
    panic!(
        "native retarget carry and reversal were not observed within {MAX_TRAJECTORY_ATTEMPTS} element epochs; last paint={:?}",
        consumer.paint.get()
    );
}

fn spring_pause_and_stop_trajectory(
    platform: &WindowsPlatform,
    consumer: &NativeSpringWindow,
    app: &mut Application,
) -> NativeSpringSubmission {
    for _ in 0..MAX_TRAJECTORY_ATTEMPTS {
        consumer.reset_paused(platform, app);
        let Some(outward) = consumer.start_outward(platform, app) else {
            continue;
        };
        let (mut source, previous) = consumer.source(app);
        source.token += 1;
        source.key_inputs += 1;
        source.playback = SpringPlayback::Paused;
        spring_pause_key(consumer);
        let paused = consumer.submitted(platform, app, source, previous, |_| true);
        if paused.value <= outward.value || paused.value >= SPRING_TARGET / 2. {
            continue;
        }
        let (source, previous) = consumer.control(app, |source| {
            source.target = 0.;
        });
        let opposite_target = consumer.submitted(platform, app, source, previous, |_| true);
        assert_eq!(opposite_target.value, paused.value);
        spring_executor_barrier(platform, app);
        let (source, previous) = consumer.control(app, |_| {});
        let held = consumer.submitted(platform, app, source, previous, |_| true);
        assert_eq!(held.value, paused.value);

        let (mut source, previous) = consumer.source(app);
        source.token += 1;
        source.key_inputs += 1;
        source.playback = SpringPlayback::Running;
        spring_pause_key(consumer);
        let resumed = consumer.submitted(platform, app, source, previous, |_| true);
        if resumed.value < held.value || resumed.value >= SPRING_TARGET / 2. {
            continue;
        }
        let Some(reversed) = consumer.carried_then_reversed(platform, app, source, resumed) else {
            continue;
        };
        let (source, previous) = consumer.control(app, |source| {
            source.playback = SpringPlayback::Stopped;
        });
        let stopped = consumer.submitted(platform, app, source, previous, |_| true);
        assert!(stopped.value <= reversed.value);
        if stopped.value < 4. {
            continue;
        }
        spring_executor_barrier(platform, app);
        let (source, previous) = consumer.control(app, |source| {
            source.target = stopped.value;
        });
        let held = consumer.submitted(platform, app, source, previous, |_| true);
        assert_eq!(held.value, stopped.value);
        let (source, previous) = consumer.control(app, |source| {
            source.playback = SpringPlayback::Running;
        });
        let resumed = consumer.submitted(platform, app, source, previous, |_| true);
        assert_eq!(resumed.value, stopped.value);
        spring_executor_barrier(platform, app);
        let (source, previous) = consumer.control(app, |_| {});
        let fixed = consumer.submitted(platform, app, source, previous, |_| true);
        assert_eq!(fixed.value, stopped.value);
        return fixed;
    }
    panic!(
        "native pause carry, reversal and zero-velocity stop were not observed within {MAX_TRAJECTORY_ATTEMPTS} element epochs; last paint={:?}",
        consumer.paint.get()
    );
}

fn spring_restore_active_trajectory(
    platform: &WindowsPlatform,
    consumer: &NativeSpringWindow,
    app: &mut Application,
) -> NativeSpringSource {
    for attempt in 0..MAX_TRAJECTORY_ATTEMPTS {
        if attempt > 0 {
            let (source, previous) = consumer.control(app, |source| {
                source.epoch += 1;
                source.target = SPRING_TARGET / 2.;
                source.playback = SpringPlayback::Running;
                source.reduced_motion = true;
            });
            let reset = consumer.submitted(platform, app, source, previous, |_| true);
            assert_eq!(reset.value, SPRING_TARGET / 2.);
        }
        let (source, previous) = consumer.control(app, |source| {
            source.target = 0.;
            source.reduced_motion = false;
        });
        let restored = consumer.submitted(platform, app, source, previous, |value| {
            value < SPRING_TARGET / 2. - 1.
        });
        if restored.value < 4. {
            continue;
        }
        let running = consumer.submitted(platform, app, source, restored.generation, |value| {
            value < restored.value - 1.
        });
        if running.value < 4. {
            continue;
        }
        assert!(running.generation > restored.generation);
        assert!(running.value > 0. && running.value < restored.value);
        return source;
    }
    panic!(
        "native reduced-motion restoration did not retain an active closing trajectory within {MAX_TRAJECTORY_ATTEMPTS} element epochs; last paint={:?}",
        consumer.paint.get()
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and a DirectX GPU"]
fn native_springs_retarget_playback_reduce_motion_and_retire_active_frames() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native Spring backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut weak_app = app.update_for_test(|cx| cx.to_async());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let consumer = NativeSpringWindow::open(&mut app, &platform);
    let root_lifetime = consumer.root.clone();
    let paint_lifetime = Rc::downgrade(&consumer.paint);
    let frame_requests = observe_frame_requests(&consumer.native);
    let (source, previous) = consumer.control(&mut app, |_| {});
    let initial = consumer.submitted(&platform, &mut app, source, previous, |_| true);
    assert_eq!(source.playback, SpringPlayback::Paused);
    assert_eq!(initial.value, 0.);
    spring_retarget_trajectory(&platform, &consumer, &mut app);
    let stopped = spring_pause_and_stop_trajectory(&platform, &consumer, &mut app);
    let (source, previous) = consumer.control(&mut app, |source| {
        source.target = SPRING_TARGET;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value > stopped.value + 1. && value < SPRING_TARGET
    });

    let (source, previous) = consumer.control(&mut app, |source| {
        source.playback = SpringPlayback::Completed;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value == SPRING_TARGET
    });
    let (source, previous) = consumer.control(&mut app, |source| {
        source.target = SPRING_TARGET / 2.;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value == SPRING_TARGET / 2.
    });
    let (source, previous) = consumer.control(&mut app, |source| {
        source.playback = SpringPlayback::Cancelled;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| value == 0.);
    let (source, previous) = consumer.control(&mut app, |source| {
        source.target = SPRING_TARGET;
        source.playback = SpringPlayback::Running;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value > 1. && value < SPRING_TARGET
    });
    let (source, previous) = consumer.control(&mut app, |source| {
        source.reduced_motion = true;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value == SPRING_TARGET
    });
    let (source, previous) = consumer.control(&mut app, |source| {
        source.target = SPRING_TARGET / 2.;
    });
    consumer.submitted(&platform, &mut app, source, previous, |value| {
        value == SPRING_TARGET / 2.
    });
    let source = spring_restore_active_trajectory(&platform, &consumer, &mut app);

    let registration = consumer.registration;
    let handle = consumer.handle;
    let native_lifetime = Rc::downgrade(&consumer.native);
    let frame_requests_before_close = frame_requests.get();
    let close_requested = Rc::new(Cell::new(false));
    app.update_for_test(|cx| {
        consumer.handle.update(cx, |_, window, _| {
            let paint = consumer.paint.clone();
            let close_requested = close_requested.clone();
            window.on_next_frame(move |window, cx| {
                let paint = paint.get().expect("the active Spring must have painted");
                assert_eq!(paint.source, source);
                assert_eq!(paint.window_id, handle.window_id());
                assert_eq!(
                    paint.previous_revision.checked_add(1),
                    Some(window.rendered_frame_revision())
                );
                assert_eq!(
                    window.presentation_facts().present_submitted_generation,
                    Some(window.rendered_frame_revision())
                );
                assert!(paint.value > 0. && paint.value < SPRING_TARGET / 2.);
                window.remove_window(cx);
                close_requested.set(true);
            });
        })
    })
    .expect("close must join the existing native frame demand");
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "active Spring exact HWND, registration and view retirement",
        Duration::from_millis(5),
        || {
            close_requested.get()
                && frame_requests.get() > frame_requests_before_close
                && !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
        },
    );
    let lifecycle = observation
        .events()
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
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
        panic!("unexpected Spring consumer retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        cx.windows().is_empty()
    }));
    let frames_after_retirement = frame_requests.get();
    drop(consumer);
    assert!(root_lifetime.upgrade().is_none());
    assert!(paint_lifetime.upgrade().is_none());
    assert!(native_lifetime.upgrade().is_none());
    spring_executor_barrier(&platform, &mut app);
    assert_eq!(frame_requests.get(), frames_after_retirement);
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        cx.windows().is_empty()
    }));
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let released = weak_app
        .update_window(AnyWindowHandle::from(handle), |_, _, _| ())
        .expect_err("Spring callbacks must not retain the App behind its weak context");
    assert_eq!(released.to_string(), "app was released");
    drop(weak_app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "native Spring platform HWND, App and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
