//! Sendable foreground work consumed by a real Windows Application, entity and renderer.
//!
//! Worker payloads and wakeups are synthetic. Production dispatch, App-local channel consumption,
//! entity effects, exact accepted/Submitted GPU captures and native retirement are exercised here.
//! This does not qualify physical input, compositor scanout or timing guarantees. Closed-queue
//! refusal retains Windows' erased-runnable policy; it does not resolve the returned Task.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use futures::{
    FutureExt, StreamExt,
    channel::{mpsc, oneshot},
};
use open_gpui::{
    Pixels, Point, SendForegroundExecutor, Subscription, Task, WeakEntity, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, black, fill, rgb,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Mutex,
    task::{Context as FutureContext, Poll, Waker},
    thread::{self, ThreadId},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeSendSource {
    Initial,
    First,
    Second,
    Cancelled,
    Retired,
}

impl NativeSendSource {
    fn colors(self) -> (u32, u32) {
        match self {
            Self::Initial => (0xff0000, 0xffffff),
            Self::First => (0x00ff00, 0xffff00),
            Self::Second => (0x0000ff, 0x00ffff),
            Self::Cancelled | Self::Retired => panic!("retired work must never paint"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeSendPayload {
    source: NativeSendSource,
    submitted_on: ThreadId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeSendReceipt {
    payload: NativeSendPayload,
    polled_on: ThreadId,
    delivered: bool,
}

#[derive(Debug, Default)]
struct NativeSendPollState {
    polls: Vec<ThreadId>,
    pending: usize,
    deliveries: Vec<bool>,
    dropped_on: Vec<ThreadId>,
}

struct NativeSendProbe {
    foreground_thread: ThreadId,
    state: Mutex<NativeSendPollState>,
    waker: Mutex<Option<Waker>>,
}

impl NativeSendProbe {
    fn poll_count(&self) -> usize {
        self.state.lock().expect("send poll state").polls.len()
    }

    fn drop_count(&self) -> usize {
        self.state.lock().expect("send drop state").dropped_on.len()
    }

    fn waker(&self) -> Waker {
        self.waker
            .lock()
            .expect("send wake state")
            .as_ref()
            .expect("a pending foreground poll must register its waker")
            .clone()
    }

    #[track_caller]
    fn assert_state(&self, polls: usize, pending: usize, deliveries: &[bool], drops: usize) {
        let state = self.state.lock().expect("send state assertions");
        assert_eq!(state.polls, vec![self.foreground_thread; polls]);
        assert_eq!(state.pending, pending);
        assert_eq!(state.deliveries, deliveries);
        assert_eq!(state.dropped_on, vec![self.foreground_thread; drops]);
    }
}

struct NativeSendFuture {
    ready: oneshot::Receiver<()>,
    sender: mpsc::UnboundedSender<NativeSendPayload>,
    payload: NativeSendPayload,
    probe: Arc<NativeSendProbe>,
}

impl Future for NativeSendFuture {
    type Output = NativeSendReceipt;

    fn poll(self: Pin<&mut Self>, cx: &mut FutureContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let polled_on = thread::current().id();
        assert_eq!(polled_on, this.probe.foreground_thread);
        this.probe
            .state
            .lock()
            .expect("send poll observation")
            .polls
            .push(polled_on);
        *this.probe.waker.lock().expect("send waker registration") = Some(cx.waker().clone());
        match Pin::new(&mut this.ready).poll(cx) {
            Poll::Pending => {
                this.probe.state.lock().expect("send pending poll").pending += 1;
                Poll::Pending
            }
            Poll::Ready(ready) => {
                ready.expect("accepted work must retain its worker gate until completion");
                let delivered = this.sender.unbounded_send(this.payload).is_ok();
                this.probe
                    .state
                    .lock()
                    .expect("send channel delivery")
                    .deliveries
                    .push(delivered);
                Poll::Ready(NativeSendReceipt {
                    payload: this.payload,
                    polled_on,
                    delivered,
                })
            }
        }
    }
}

impl Drop for NativeSendFuture {
    fn drop(&mut self) {
        self.probe.waker.lock().expect("send waker release").take();
        self.probe
            .state
            .lock()
            .expect("send future release")
            .dropped_on
            .push(thread::current().id());
    }
}

struct NativeSendSubmission {
    task: Task<NativeSendReceipt>,
    release: oneshot::Sender<()>,
    payload: NativeSendPayload,
    probe: Arc<NativeSendProbe>,
}

fn submit_send_work(
    executor: &SendForegroundExecutor,
    sender: &mpsc::UnboundedSender<NativeSendPayload>,
    source: NativeSendSource,
) -> NativeSendSubmission {
    let (release, ready) = oneshot::channel();
    let probe = Arc::new(NativeSendProbe {
        foreground_thread: thread::current().id(),
        state: Mutex::default(),
        waker: Mutex::default(),
    });
    let executor = executor.clone();
    let sender = sender.clone();
    let worker_probe = probe.clone();
    let (task, payload) = thread::spawn(move || {
        let payload = NativeSendPayload {
            source,
            submitted_on: thread::current().id(),
        };
        let task = executor.spawn(NativeSendFuture {
            ready,
            sender,
            payload,
            probe: worker_probe,
        });
        (task, payload)
    })
    .join()
    .expect("worker submission through the official sendable foreground executor");
    assert_ne!(payload.submitted_on, probe.foreground_thread);
    probe.assert_state(0, 0, &[], 0);
    assert!(!task.is_ready());
    NativeSendSubmission {
        task,
        release,
        payload,
        probe,
    }
}

fn wake_send_work(waker: Waker) {
    thread::spawn(move || waker.wake())
        .join()
        .expect("a worker must wake the pending foreground future");
}

fn release_send_work(release: oneshot::Sender<()>) {
    thread::spawn(move || release.send(()))
        .join()
        .expect("worker completion must not panic")
        .expect("pending foreground work must still own its gate receiver");
}

fn wait_for_send_polls(platform: &WindowsPlatform, probe: &NativeSendProbe, polls: usize) {
    let trace = pump_messages_until_with_delay(
        "send future reaches its next poll through the actual Windows foreground wake",
        Duration::from_millis(5),
        || probe.poll_count() >= polls,
    );
    assert_eq!(probe.poll_count(), polls);
    assert!(trace.contains(
        platform.handle,
        crate::WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD
    ));
}

fn shutdown_send_app(app: &mut Application) {
    app.update_for_test(|cx| cx.shutdown_for_native_exit_test());
    pump_messages_until_with_delay(
        "send consumer terminal App shutdown authority",
        Duration::from_millis(5),
        || app.update_for_test(|cx| cx.native_exit_authority_is_settled_for_test()),
    );
}

#[derive(Clone, Copy, Debug)]
struct NativeSendPaint {
    window_id: WindowId,
    previous_revision: u64,
    payload: NativeSendPayload,
    bounds: Bounds<Pixels>,
}

struct NativeSendRoot {
    payload: NativeSendPayload,
    paint: Rc<Cell<Option<NativeSendPaint>>>,
    _receiver: Task<()>,
}

impl Render for NativeSendRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let payload = self.payload;
        let (color, marker_color) = payload.source.colors();
        let paint = self.paint.clone();
        div()
            .relative()
            .size_full()
            .bg(black())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(color)));
                        paint.set(Some(NativeSendPaint {
                            window_id: window.window_handle().window_id(),
                            previous_revision: window.rendered_frame_revision(),
                            payload,
                            bounds,
                        }));
                    },
                )
                .absolute()
                .left(px(16.))
                .top(px(16.))
                .w(px(96.))
                .h(px(64.)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .top(px(96.))
                    .w(px(96.))
                    .h(px(24.))
                    .bg(rgb(marker_color)),
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeSendEffect {
    Consumed(NativeSendPayload, ThreadId),
    Notified(NativeSendPayload, ThreadId),
    Deferred(NativeSendPayload, ThreadId),
    Released(NativeSendPayload, ThreadId),
}

struct NativeSendWindow {
    handle: WindowHandle<NativeSendRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    root: WeakEntity<NativeSendRoot>,
    paint: Rc<Cell<Option<NativeSendPaint>>>,
    sender: mpsc::UnboundedSender<NativeSendPayload>,
    effects: Rc<RefCell<Vec<NativeSendEffect>>>,
    receiver_lifetime: std::rc::Weak<()>,
    _observers: [Subscription; 2],
}

impl NativeSendWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let paint = Rc::new(Cell::new(None));
        let effects = Rc::new(RefCell::new(Vec::new()));
        let capture = Rc::new(());
        let receiver_lifetime = Rc::downgrade(&capture);
        let started = Rc::new(Cell::new(false));
        let main_thread = thread::current().id();
        let (sender, mut receiver) = mpsc::unbounded();
        let handle = app
            .update_for_test(|cx| {
                assert!(!cx.is_test());
                assert!(!cx.background_executor().is_test());
                assert!(!cx.foreground_executor().scheduler_executor().is_test());
                assert!(cx.background_executor().is_main_thread());
                assert!(!cx.to_async().is_test());
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(200.), px(160.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        inactive_frame_interval: None,
                        low_power_frame_interval: None,
                        thermal_frame_interval: None,
                        ..WindowOptions::default()
                    },
                    |_, cx| {
                        cx.new(|cx| {
                            let consumed = effects.clone();
                            let receiver_started = started.clone();
                            let task = cx.spawn(async move |root, cx| {
                                let _keep_alive = capture;
                                assert_eq!(thread::current().id(), main_thread);
                                receiver_started.set(true);
                                while let Some(payload) = receiver.next().await {
                                    assert_eq!(thread::current().id(), main_thread);
                                    cx.update(|cx| {
                                        consumed.borrow_mut().push(NativeSendEffect::Consumed(
                                            payload,
                                            thread::current().id(),
                                        ));
                                        root.update(cx, |root: &mut NativeSendRoot, cx| {
                                            root.payload = payload;
                                            cx.notify();
                                        })
                                        .expect("App-local receiver weak entity update");
                                        let root = root.clone();
                                        let effects = consumed.clone();
                                        cx.defer(move |cx| {
                                            let payload = root
                                                .read_with(cx, |root, _| root.payload)
                                                .expect("deferred effect entity source");
                                            effects.borrow_mut().push(NativeSendEffect::Deferred(
                                                payload,
                                                thread::current().id(),
                                            ));
                                        });
                                    });
                                }
                            });
                            NativeSendRoot {
                                payload: NativeSendPayload {
                                    source: NativeSendSource::Initial,
                                    submitted_on: main_thread,
                                },
                                paint: paint.clone(),
                                _receiver: task,
                            }
                        })
                    },
                )
            })
            .expect("the real send-executor consumer window must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("the send consumer must have its exact native registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("the send consumer HWND must have its real native owner");
        assert_eq!(native.registration, registration);
        let (root, observers) = app.update_for_test(|cx| {
            let root = handle.root(cx).expect("the send consumer's actual entity");
            let notified = effects.clone();
            let observer = cx.observe(&root, move |root, cx| {
                notified.borrow_mut().push(NativeSendEffect::Notified(
                    root.read(cx).payload,
                    thread::current().id(),
                ));
            });
            let released = effects.clone();
            let release = cx.observe_release(&root, move |root, _| {
                released.borrow_mut().push(NativeSendEffect::Released(
                    root.payload,
                    thread::current().id(),
                ));
            });
            (root.downgrade(), [observer, release])
        });
        pump_messages_until_with_delay(
            "the existing App-local receiver registers its pending channel await",
            Duration::from_millis(5),
            || started.get(),
        );
        assert!(effects.borrow().is_empty());
        Self {
            handle,
            registration,
            native,
            root,
            paint,
            sender,
            effects,
            receiver_lifetime,
            _observers: observers,
        }
    }

    fn submitted(
        &self,
        app: &mut Application,
        payload: NativeSendPayload,
        previous: u64,
    ) -> WindowFrameCapture {
        let mut submitted = None;
        let mut last_state = None;
        let description =
            format!("send payload {payload:?} reaches exact Submitted pixels after {previous}");
        pump_messages_until_with_delay(&description, Duration::from_millis(5), || {
            submitted = app
                .update_for_test(|cx| {
                    self.handle.update(cx, |root, window, _| {
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
                            && paint.payload == payload
                            && paint.previous_revision.checked_add(1) == Some(attempt.generation)
                            && paint.bounds.origin == point(px(16.), px(16.))
                            && paint.bounds.size == size(px(96.), px(64.)))
                        {
                            return None;
                        }
                        assert_eq!(root.payload, payload);
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
                        let image = capture
                            .image
                            .as_ref()
                            .expect("submitted send-consumer RGBA");
                        assert_eq!(
                            image.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        assert_eq!(
                            window
                                .render_to_image()
                                .expect("native send-consumer Scene GPU readback"),
                            *image
                        );
                        let (color, marker_color) = payload.source.colors();
                        for (position, color) in [
                            (paint.bounds.center(), color),
                            (point(px(64.), px(108.)), marker_color),
                            (point(px(144.), px(64.)), 0),
                        ] {
                            assert_send_pixel(&capture, position, color);
                        }
                        assert_eq!(window.rendered_frame_revision(), attempt.generation);
                        assert_eq!(
                            window.presentation_facts().latest_present_attempt,
                            facts.latest_present_attempt
                        );
                        Some(capture)
                    })
                })
                .expect("the actual send consumer must remain live through exact-frame capture");
            submitted.is_some()
        });
        submitted.unwrap_or_else(|| {
            panic!("send payload did not reach Submitted pixels: {last_state:?}")
        })
    }

    fn retire(
        self,
        app: &mut Application,
        platform: &WindowsPlatform,
        observation: &NativeWindowTestObservation,
    ) {
        let native_lifetime = Rc::downgrade(&self.native);
        let paint_lifetime = Rc::downgrade(&self.paint);
        app.update_for_test(|cx| {
            self.handle
                .update(cx, |_, window, cx| window.remove_window(cx))
        })
        .expect("the send consumer must retire through its existing public window owner");
        pump_messages_until_with_delay(
            "send consumer exact HWND, registration, entity and local callback retirement",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(self.registration.as_raw())).as_bool() }
                    && !platform
                        .raw_window_handles
                        .read()
                        .contains(&self.registration)
                    && platform.inner.native_retirement_is_settled()
                    && self.root.upgrade().is_none()
                    && self.receiver_lifetime.upgrade().is_none()
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
            panic!("unexpected send consumer retirement: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
        app.update_for_test(|cx| {
            assert!(self.handle.update(cx, |_, _, _| ()).is_err());
            assert!(cx.windows().is_empty());
        });
        assert!(self.sender.is_closed());
        assert!(platform.raw_window_handles.read().is_empty());
        drop(self);
        assert!(native_lifetime.upgrade().is_none());
        assert!(paint_lifetime.upgrade().is_none());
    }
}

fn assert_send_pixel(capture: &WindowFrameCapture, position: Point<Pixels>, color: u32) {
    let image = capture
        .image
        .as_ref()
        .expect("native send-consumer GPU pixels");
    let x = (f32::from(position.x) * capture.metadata.scale_factor).floor() as u32;
    let y = (f32::from(position.y) * capture.metadata.scale_factor).floor() as u32;
    assert!(x < image.width() && y < image.height());
    assert_eq!(
        image.get_pixel(x, y).0,
        [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255],
        "send consumer generation {} at {position:?}",
        capture.metadata.framework_frame_generation,
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and a DirectX GPU"]
fn native_send_foreground_app_view_repolls_cancels_submits_and_releases() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(WindowsPlatform::new(false).expect("native send-executor backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let inner_lifetime = Rc::downgrade(&platform.inner);
    let dispatcher_lifetime = Arc::downgrade(&platform.inner.dispatcher);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut weak_app = app.update_for_test(|cx| cx.to_async());
    let executor = app.update_for_test(|cx| cx.foreground_executor().to_send());
    platform.begin_vsync_thread();
    let consumer = NativeSendWindow::open(&mut app, &platform);
    let handle = consumer.handle;
    let root_lifetime = consumer.root.clone();
    let receiver_lifetime = consumer.receiver_lifetime.clone();
    let sender = consumer.sender.clone();
    let effects = consumer.effects.clone();
    let main_thread = thread::current().id();
    let mut previous = consumer.submitted(
        &mut app,
        NativeSendPayload {
            source: NativeSendSource::Initial,
            submitted_on: main_thread,
        },
        0,
    );
    let mut expected_effects = Vec::new();
    for source in [NativeSendSource::First, NativeSendSource::Second] {
        let NativeSendSubmission {
            task,
            release,
            payload,
            probe,
        } = submit_send_work(&executor, &sender, source);
        wait_for_send_polls(&platform, &probe, 1);
        probe.assert_state(1, 1, &[], 0);
        assert!(!task.is_ready());
        wake_send_work(probe.waker());
        wait_for_send_polls(&platform, &probe, 2);
        probe.assert_state(2, 2, &[], 0);
        assert!(!task.is_ready());
        assert_eq!(*effects.borrow(), expected_effects);
        release_send_work(release);
        wait_for_send_polls(&platform, &probe, 3);
        let submitted = consumer.submitted(
            &mut app,
            payload,
            previous.metadata.framework_frame_generation,
        );
        expected_effects.extend([
            NativeSendEffect::Consumed(payload, main_thread),
            NativeSendEffect::Notified(payload, main_thread),
            NativeSendEffect::Deferred(payload, main_thread),
        ]);
        assert_eq!(*effects.borrow(), expected_effects);
        probe.assert_state(3, 2, &[true], 1);
        assert!(task.is_ready());
        assert_eq!(
            task.now_or_never(),
            Some(NativeSendReceipt {
                payload,
                polled_on: main_thread,
                delivered: true,
            })
        );
        assert_ne!(submitted.image, previous.image);
        previous = submitted;
    }

    let NativeSendSubmission {
        task,
        release,
        probe,
        ..
    } = submit_send_work(&executor, &sender, NativeSendSource::Cancelled);
    wait_for_send_polls(&platform, &probe, 1);
    let stale_waker = probe.waker();
    thread::spawn(move || drop(task))
        .join()
        .expect("worker task cancellation");
    pump_messages_until_with_delay(
        "cancelled pending send work releases its future on the foreground thread",
        Duration::from_millis(5),
        || probe.drop_count() == 1,
    );
    probe.assert_state(1, 1, &[], 1);
    assert!(release.is_canceled());
    assert!(release.send(()).is_err());
    wake_send_work(stale_waker);
    pump_messages_until_idle("cancelled send future stale wake");
    probe.assert_state(1, 1, &[], 1);
    assert_eq!(*effects.borrow(), expected_effects);

    let NativeSendSubmission {
        task,
        release,
        payload,
        probe,
    } = submit_send_work(&executor, &sender, NativeSendSource::Retired);
    wait_for_send_polls(&platform, &probe, 1);
    probe.assert_state(1, 1, &[], 0);
    let last_payload = app.update_for_test(|cx| {
        consumer
            .root
            .read_with(cx, |root, _| root.payload)
            .expect("live consumer source")
    });
    consumer.retire(&mut app, &platform, &observation);
    expected_effects.push(NativeSendEffect::Released(last_payload, main_thread));
    assert_eq!(*effects.borrow(), expected_effects);
    assert!(root_lifetime.upgrade().is_none());
    assert!(receiver_lifetime.upgrade().is_none());
    assert!(sender.is_closed());
    release_send_work(release);
    wait_for_send_polls(&platform, &probe, 2);
    probe.assert_state(2, 1, &[false], 1);
    assert_eq!(
        task.now_or_never(),
        Some(NativeSendReceipt {
            payload,
            polled_on: main_thread,
            delivered: false,
        })
    );
    assert_eq!(*effects.borrow(), expected_effects);
    assert!(root_lifetime.upgrade().is_none());
    assert!(platform.raw_window_handles.read().is_empty());

    shutdown_send_app(&mut app);
    drop(app);
    let released = weak_app
        .update_window(AnyWindowHandle::from(handle), |_, _, _| ())
        .expect_err("send work and its App-local receiver must not retain the App");
    assert_eq!(released.to_string(), "app was released");
    drop(weak_app);
    drop(executor);
    drop(sender);
    pump_messages_until_idle("send consumer final foreground drain");
    assert!(platform.inner.main_receiver.is_empty());
    drop(platform);
    pump_messages_until_with_delay(
        "send consumer platform HWND, App, dispatcher and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && inner_lifetime.upgrade().is_none()
                && dispatcher_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(*effects.borrow(), expected_effects);
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
}

/// Closed-queue wakes and fresh submissions retain their erased runnables and unfinished Tasks.
/// With the profiler enabled, reconstruction on the same thread also checks refusal accounting.
#[test]
fn native_send_foreground_closed_dispatcher_refuses_without_resolving_task() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("native send refusal backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let inner_lifetime = Rc::downgrade(&platform.inner);
    let dispatcher = platform.inner.dispatcher.clone();
    let dispatcher_lifetime = Arc::downgrade(&dispatcher);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let executor = app.update_for_test(|cx| cx.foreground_executor().to_send());
    let (sender, receiver) = mpsc::unbounded();
    pump_messages_until_idle("send refusal initial foreground drain");
    assert!(platform.inner.main_receiver.is_empty());
    let held = submit_send_work(&executor, &sender, NativeSendSource::Retired);
    wait_for_send_polls(&platform, &held.probe, 1);
    held.probe.assert_state(1, 1, &[], 0);
    assert!(platform.inner.main_receiver.is_empty());
    shutdown_send_app(&mut app);
    pump_messages_until_idle("send refusal final foreground drain");
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "send refusal releases every real Windows foreground queue receiver",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && inner_lifetime.upgrade().is_none()
        },
    );
    release_send_work(held.release);
    held.probe.assert_state(1, 1, &[], 0);
    assert!(!held.task.is_ready());
    let late = submit_send_work(&executor, &sender, NativeSendSource::Retired);
    pump_messages_until_idle("closed Windows dispatcher cannot poll late send submissions");
    late.probe.assert_state(0, 0, &[], 0);
    assert!(!late.task.is_ready());
    assert!(
        !dispatcher
            .wake_posted
            .load(std::sync::atomic::Ordering::Acquire)
    );
    drop(held.task);
    drop(late.task);
    drop(late.release);
    drop(receiver);
    assert!(sender.is_closed());
    held.probe.assert_state(1, 1, &[], 0);
    late.probe.assert_state(0, 0, &[], 0);
    drop(executor);
    drop(dispatcher);
    assert!(dispatcher_lifetime.upgrade().is_some());
    assert!(platform_lifetime.upgrade().is_none());
    assert!(inner_lifetime.upgrade().is_none());
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);

    #[cfg(feature = "profiler")]
    {
        use open_gpui::profiler::journal::{
            ForegroundEvent, ForegroundJournalEntry, IntervalBoundary, TASK_POLL_FLOOR,
        };

        let replacement_platform = Rc::new(
            WindowsPlatform::new(true).expect("reconstructed native send-executor backend"),
        );
        let replacement_hwnd = replacement_platform.handle;
        let replacement_lifetime = Rc::downgrade(&replacement_platform.inner);
        let replacement_dispatcher = Arc::downgrade(&replacement_platform.inner.dispatcher);
        let replacement_lifecycle = replacement_platform.lifecycle_test_probe.clone();
        let mut replacement = Application::with_platform(replacement_platform.clone())
            .with_quit_mode(QuitMode::Explicit);
        let mut collector = replacement.update_for_test(|cx| cx.foreground_journal().collector());
        let completed = replacement.update_for_test(|cx| {
            cx.foreground_executor().spawn(async {
                thread::sleep(TASK_POLL_FLOOR + Duration::from_millis(1));
                42
            })
        });
        let trace = pump_messages_until_with_delay(
            "reconstructed Windows foreground task completes after rejected send work",
            Duration::from_millis(5),
            || completed.is_ready(),
        );
        assert!(trace.contains(
            replacement_hwnd,
            crate::WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD
        ));
        assert_eq!(completed.now_or_never(), Some(42));
        pump_messages_until_idle("reconstructed send-executor idle accounting barrier");
        assert!(replacement_platform.inner.main_receiver.is_empty());
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(drained.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Event(ForegroundEvent::TaskPoll(_))
        )));
        assert!(
            drained.entries.iter().any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
            )),
            "refused send work must settle its original foreground queue counter"
        );
        shutdown_send_app(&mut replacement);
        drop(replacement);
        pump_messages_until_idle("reconstructed send-executor final foreground drain");
        assert!(replacement_platform.inner.main_receiver.is_empty());
        drop(replacement_platform);
        pump_messages_until_with_delay(
            "reconstructed send-executor platform and dispatcher release",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(replacement_hwnd)).as_bool() }
                    && replacement_lifetime.upgrade().is_none()
                    && replacement_dispatcher.upgrade().is_none()
            },
        );
        assert_eq!(replacement_lifecycle.platform_destroy_attempts(), 1);
        assert_eq!(replacement_lifecycle.ole_uninitialize_count(), 1);
    }
}
