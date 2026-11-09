//! Notification responses over the real Windows foreground dispatcher and native retirement.
//!
//! Responses are synthetic channel inputs; these tests never register an AUMID, display a toast,
//! or require user clicks. They qualify callback ordering, replacement, reentry, and teardown,
//! without claiming operating-system notification delivery or native activation fidelity.

use super::*;
use open_gpui::{SystemNotification, SystemNotificationResponse};
use std::thread::{self, ThreadId};

fn response(tag: &str, action_id: Option<&str>) -> SystemNotificationResponse {
    SystemNotificationResponse {
        tag: tag.to_string().into(),
        action_id: action_id.map(|id| id.to_string().into()),
    }
}

#[test]
fn native_system_notification_callbacks_preserve_order_main_thread_and_replacement() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("native notification backend"));
    let hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let main_thread = thread::current().id();
    let delivered = Rc::new(RefCell::new(Vec::<(
        usize,
        SystemNotificationResponse,
        ThreadId,
    )>::new()));
    let sender = platform
        .system_notifications
        .borrow()
        .response_sender_for_test();
    let worker_sender = sender.clone();
    let worker_thread = thread::spawn(move || {
        worker_sender
            .unbounded_send(response(
                "queued-action",
                Some("open?file=日本語&mode=read"),
            ))
            .expect("queued background action response");
        worker_sender
            .unbounded_send(response("queued-body", None))
            .expect("queued background body response");
        thread::current().id()
    })
    .join()
    .expect("synthetic native activation producer");
    assert_ne!(worker_thread, main_thread);
    assert!(delivered.borrow().is_empty());

    let unused_capture = Rc::new(());
    let unused_lifetime = Rc::downgrade(&unused_capture);
    platform.on_system_notification_response(Box::new(move |_| {
        let _keep_alive = &unused_capture;
        panic!("replaced notification callback must never execute");
    }));

    let first_capture = Rc::new(());
    let first_lifetime = Rc::downgrade(&first_capture);
    let replacement_capture = Rc::new(());
    let replacement_lifetime = Rc::downgrade(&replacement_capture);
    let first_delivered = delivered.clone();
    let callback_platform = Rc::downgrade(&platform);
    let reentrant_sender = sender.clone();
    platform.on_system_notification_response(Box::new(move |notification_response| {
        let _keep_alive = &first_capture;
        first_delivered
            .borrow_mut()
            .push((1, notification_response, thread::current().id()));
        let platform = callback_platform.upgrade().expect("live response platform");
        platform.dismiss_system_notification("queued-action");
        let replacement_delivered = first_delivered.clone();
        let replacement_capture = replacement_capture.clone();
        platform.on_system_notification_response(Box::new(move |notification_response| {
            let _keep_alive = &replacement_capture;
            replacement_delivered.borrow_mut().push((
                2,
                notification_response,
                thread::current().id(),
            ));
        }));
        reentrant_sender
            .unbounded_send(response("reentrant", Some("continue")))
            .expect("response queued from a reentrant callback");
    }));
    assert!(unused_lifetime.upgrade().is_none());
    assert!(first_lifetime.upgrade().is_some());
    assert!(delivered.borrow().is_empty());

    pump_messages_until("ordered native notification responses", || {
        delivered.borrow().len() == 3
    });
    assert_eq!(
        delivered.borrow().as_slice(),
        &[
            (
                1,
                response("queued-action", Some("open?file=日本語&mode=read")),
                main_thread,
            ),
            (2, response("queued-body", None), main_thread),
            (2, response("reentrant", Some("continue")), main_thread),
        ],
    );
    assert!(first_lifetime.upgrade().is_none());
    assert!(replacement_lifetime.upgrade().is_some());

    drop(platform);
    assert!(sender.is_closed());
    assert!(replacement_lifetime.upgrade().is_none());
    pump_messages_until_with_delay(
        "notification callback and exact native platform retirement",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && lifecycle.ole_uninitialize_count() == 1
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(delivered.borrow().len(), 3);
}

#[test]
fn native_system_notification_retirement_cancels_queued_delivery_before_native_retry() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("notification retirement backend"));
    let hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let delivered = Rc::new(Cell::new(0));
    let callback_delivered = delivered.clone();
    let capture = Rc::new(());
    let callback_lifetime = Rc::downgrade(&capture);
    platform.on_system_notification_response(Box::new(move |_| {
        let _keep_alive = &capture;
        callback_delivered.set(callback_delivered.get() + 1);
    }));
    let sender = platform
        .system_notifications
        .borrow()
        .response_sender_for_test();
    sender
        .unbounded_send(response("queued-before-retirement", Some("open")))
        .expect("queued notification before retirement");
    lifecycle.fail_next_platform_destroy();
    lifecycle.fail_next_retirement_timer.set(true);
    platform.begin_native_finalization();

    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 0);
    assert!(callback_lifetime.upgrade().is_none());
    assert!(sender.is_closed());
    assert!(sender.unbounded_send(response("late", None)).is_err());

    let rejected_capture = Rc::new(());
    let rejected_lifetime = Rc::downgrade(&rejected_capture);
    platform.on_system_notification_response(Box::new(move |_| {
        let _keep_alive = &rejected_capture;
        panic!("retired platform must reject callback registration");
    }));
    assert!(rejected_lifetime.upgrade().is_none());
    platform.set_app_identity(
        "OpenGpui.RetiredNotificationTest",
        "Retired notification test",
    );
    assert!(platform.app_identity.borrow().is_none());
    platform.show_system_notification(SystemNotification {
        tag: "retired-no-display".into(),
        title: "Retired".into(),
        body: "Retirement rejects native construction".into(),
        actions: Vec::new(),
    });
    platform.dismiss_system_notification("retired-no-display");

    assert_eq!(platform.inner.run_foreground_task(), Some(0));
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    assert_eq!(lifecycle.platform_destroy_attempts(), 2);
    assert!(platform.inner.native_retirement_is_settled());
    assert_eq!(delivered.get(), 0);
    drop(platform);
    pump_messages_until(
        "cancelled notification task releases native platform",
        || platform_lifetime.upgrade().is_none(),
    );
    assert_eq!(delivered.get(), 0);
}

#[test]
fn native_system_notification_reentrant_retirement_does_not_restore_running_callback() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("reentrant notification backend"));
    let hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let delivered = Rc::new(RefCell::new(Vec::new()));
    let callback_delivered = delivered.clone();
    let capture = Rc::new(());
    let callback_lifetime = Rc::downgrade(&capture);
    let callback_platform = Rc::downgrade(&platform);
    platform.on_system_notification_response(Box::new(move |notification_response| {
        let _keep_alive = &capture;
        callback_delivered.borrow_mut().push(notification_response);
        callback_platform
            .upgrade()
            .expect("platform during running notification callback")
            .begin_native_finalization();
    }));
    let sender = platform
        .system_notifications
        .borrow()
        .response_sender_for_test();
    sender.unbounded_send(response("retire-now", None)).unwrap();
    sender
        .unbounded_send(response("never-deliver", Some("open")))
        .unwrap();
    lifecycle.fail_next_platform_destroy();
    lifecycle.fail_next_retirement_timer.set(true);

    pump_messages_until_with_delay(
        "reentrant notification retirement releases callback and native retry transport",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && lifecycle.ole_uninitialize_count() == 1
                && callback_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(
        delivered.borrow().as_slice(),
        &[response("retire-now", None)]
    );
    assert!(sender.is_closed());
    assert_eq!(lifecycle.platform_destroy_attempts(), 2);
    assert!(platform.inner.native_retirement_is_settled());
    drop(platform);
    assert!(platform_lifetime.upgrade().is_none());
}

use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    Pixels, Point, WindowFrameCapture, WindowHandle, WindowInitialPresentationStatus, black, fill,
    rgb,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeNotificationSource {
    Initial,
    Body,
    Action,
}

impl NativeNotificationSource {
    fn colors(self) -> (u32, u32) {
        match self {
            Self::Initial => (0xff0000, 0xffffff),
            Self::Body => (0x00ff00, 0xffff00),
            Self::Action => (0x0000ff, 0x00ffff),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeNotificationPaint {
    window_id: WindowId,
    previous_revision: u64,
    source: NativeNotificationSource,
    bounds: Bounds<Pixels>,
}

struct NativeNotificationRoot {
    source: NativeNotificationSource,
    paint: Rc<Cell<Option<NativeNotificationPaint>>>,
}

impl Render for NativeNotificationRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source;
        let (color, marker_color) = source.colors();
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
                        paint.set(Some(NativeNotificationPaint {
                            window_id: window.window_handle().window_id(),
                            previous_revision: window.rendered_frame_revision(),
                            source,
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

#[derive(Debug, Eq, PartialEq)]
enum NativeNotificationEffect {
    Response(SystemNotificationResponse, ThreadId),
    Notified(NativeNotificationSource),
    Deferred(NativeNotificationSource),
}

fn assert_notification_pixel(capture: &WindowFrameCapture, position: Point<Pixels>, color: u32) {
    let image = capture
        .image
        .as_ref()
        .expect("native notification RGBA readback");
    let x = (f32::from(position.x) * capture.metadata.scale_factor).floor() as u32;
    let y = (f32::from(position.y) * capture.metadata.scale_factor).floor() as u32;
    assert!(x < image.width() && y < image.height());
    assert_eq!(
        image.get_pixel(x, y).0,
        [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255],
        "notification generation {} at {position:?}",
        capture.metadata.framework_frame_generation,
    );
}

fn notification_submitted_frame(
    app: &mut Application,
    handle: WindowHandle<NativeNotificationRoot>,
    paint: &Cell<Option<NativeNotificationPaint>>,
    source: NativeNotificationSource,
    previous: u64,
) -> WindowFrameCapture {
    let mut submitted = None;
    let description =
        format!("notification {source:?} reaches exact nonempty Submitted pixels after {previous}");
    pump_messages_until_with_delay(&description, Duration::from_millis(5), || {
        submitted = app
            .update_for_test(|cx| {
                handle.update(cx, |root, window, _| {
                    let facts = window.presentation_facts();
                    let paint = paint.get()?;
                    let attempt = facts.latest_present_attempt?;
                    if !(attempt.generation > previous
                        && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                        && attempt.contained_valid_primitives
                        && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                        && facts.native_visible
                        && facts.frame_accepted_generation == Some(attempt.generation)
                        && facts.present_submitted_generation == Some(attempt.generation)
                        && facts.non_empty_presented_generation == Some(attempt.generation)
                        && window.rendered_frame_revision() == attempt.generation
                        && window.submitted_frame_diagnostics_for_test()?.generation()
                            == attempt.generation
                        && paint.window_id == handle.window_id()
                        && paint.source == source
                        && paint.previous_revision.checked_add(1) == Some(attempt.generation)
                        && paint.bounds.origin == point(px(16.), px(16.))
                        && paint.bounds.size == size(px(96.), px(64.)))
                    {
                        return None;
                    }
                    assert_eq!(root.source, source);
                    let capture = window.capture_frame();
                    assert!(capture.unsupported_reason.is_none(), "{capture:?}");
                    assert_eq!(capture.metadata.window_id, handle.window_id());
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
                        .expect("submitted notification pixels");
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
                            .expect("native notification Scene readback"),
                        *image
                    );
                    let (color, marker_color) = source.colors();
                    assert_notification_pixel(&capture, paint.bounds.center(), color);
                    assert_notification_pixel(&capture, point(px(64.), px(108.)), marker_color);
                    assert_notification_pixel(&capture, point(px(144.), px(64.)), 0);
                    assert_eq!(window.rendered_frame_revision(), attempt.generation);
                    assert_eq!(
                        window.presentation_facts().latest_present_attempt,
                        facts.latest_present_attempt
                    );
                    Some(capture)
                })
            })
            .expect("the notification consumer must remain live through exact-frame capture");
        submitted.is_some()
    });
    submitted.expect("the notification source must reach a newer Submitted generation")
}

#[test]
fn native_system_notification_app_view_submits_body_and_action_then_releases() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(WindowsPlatform::new(false).expect("native notification App backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let platform_inner_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut weak_app = app.update_for_test(|cx| cx.to_async());
    platform.begin_vsync_thread();
    let paint = Rc::new(Cell::new(None));
    let paint_lifetime = Rc::downgrade(&paint);
    let handle = app
        .update_for_test(|cx| {
            assert!(!cx.is_test());
            assert!(!cx.background_executor().is_test());
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
                    cx.new(|_| NativeNotificationRoot {
                        source: NativeNotificationSource::Initial,
                        paint: paint.clone(),
                    })
                },
            )
        })
        .expect("the actual notification Render consumer must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("the notification window must have its exact HWND registration");
    let native = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the notification HWND must have a native owner");
    assert_eq!(native.registration, registration);
    let native_lifetime = Rc::downgrade(&native);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (root, observer) = app.update_for_test(|cx| {
        let root = handle
            .root(cx)
            .expect("the notification window must expose its real entity");
        let observed = events.clone();
        let observer = cx.observe(&root, move |root, cx| {
            observed
                .borrow_mut()
                .push(NativeNotificationEffect::Notified(root.read(cx).source));
        });
        (root.downgrade(), observer)
    });
    let capture = Rc::new(());
    let callback_lifetime = Rc::downgrade(&capture);
    let main_thread = thread::current().id();
    app.update_for_test(|cx| {
        let callback_root = root.clone();
        let delivered = events.clone();
        cx.on_system_notification_response(move |activation, app| {
            let _keep_alive = &capture;
            assert_eq!(thread::current().id(), main_thread);
            assert_eq!(activation.tag.as_ref(), "native-view");
            delivered
                .borrow_mut()
                .push(NativeNotificationEffect::Response(
                    activation.clone(),
                    thread::current().id(),
                ));
            callback_root
                .update(app, |root, cx| {
                    root.source = match activation.action_id.as_deref() {
                        None => NativeNotificationSource::Body,
                        Some("open?file=日本語&mode=read") => NativeNotificationSource::Action,
                        other => panic!("unexpected notification action: {other:?}"),
                    };
                    cx.notify();
                })
                .expect("a response must update the native window's weak entity");
            let root = callback_root.clone();
            let delivered = delivered.clone();
            app.defer(move |app| {
                let source = root
                    .read_with(app, |root, _| root.source)
                    .expect("the notification deferred effect must read the updated entity");
                delivered
                    .borrow_mut()
                    .push(NativeNotificationEffect::Deferred(source));
            });
        });
    });
    let sender = platform
        .system_notifications
        .borrow()
        .response_sender_for_test();
    let mut previous = notification_submitted_frame(
        &mut app,
        handle,
        &paint,
        NativeNotificationSource::Initial,
        0,
    );
    let response_kind =
        NativeBoundaryKind::Callback(NativeCallbackKind::SystemNotificationResponse);
    let mut expected_events = Vec::new();
    for (activation, source) in [
        (
            response("native-view", None),
            NativeNotificationSource::Body,
        ),
        (
            response("native-view", Some("open?file=日本語&mode=read")),
            NativeNotificationSource::Action,
        ),
    ] {
        app.update_for_test(|cx| {
            sender
                .unbounded_send(activation.clone())
                .expect("response while App is borrowed");
            assert_eq!(platform.inner.run_foreground_task(), Some(0));
            assert_eq!(events.borrow().as_slice(), expected_events.as_slice());
            let diagnostics =
                cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default());
            let pending = diagnostics
                .pending
                .iter()
                .filter(|entry| entry.kind == response_kind)
                .collect::<Vec<_>>();
            assert_eq!(
                pending.len(),
                1,
                "the native response must wait for the active App borrow"
            );
            assert_eq!(pending[0].target, NativeBoundaryTarget::Application);
        });
        let submitted = notification_submitted_frame(
            &mut app,
            handle,
            &paint,
            source,
            previous.metadata.framework_frame_generation,
        );
        expected_events.extend([
            NativeNotificationEffect::Response(activation, main_thread),
            NativeNotificationEffect::Notified(source),
            NativeNotificationEffect::Deferred(source),
        ]);
        assert_eq!(events.borrow().as_slice(), expected_events.as_slice());
        assert_ne!(submitted.image, previous.image);
        previous = submitted;
    }
    app.update_for_test(|cx| {
        let diagnostics = cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default());
        let responses = diagnostics
            .terminal
            .iter()
            .filter(|entry| entry.kind == response_kind)
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.omitted_before_cursor, 0);
        assert_eq!(responses.len(), 2);
        assert!(responses.iter().all(|entry| entry.disposition
            == NativeBoundaryDisposition::Delivered { input_result: None }));
        assert!(responses[0].sequence < responses[1].sequence);
        assert!(
            diagnostics
                .pending
                .iter()
                .all(|entry| entry.kind != response_kind)
        );
    });

    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the notification window must close through its public owner");
    pump_messages_until_with_delay(
        "notification exact HWND, registration and entity retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
                && root.upgrade().is_none()
        },
    );
    let retirement = observation
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
    ] = retirement.as_slice()
    else {
        panic!("unexpected notification consumer retirement: {retirement:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    drop(native);
    assert!(native_lifetime.upgrade().is_none());
    drop(observer);
    drop(paint);
    assert!(paint_lifetime.upgrade().is_none());
    app.update_for_test(|cx| cx.shutdown_for_native_exit_test());
    pump_messages_until_with_delay(
        "notification terminal App shutdown authority",
        Duration::from_millis(5),
        || app.update_for_test(|cx| cx.native_exit_authority_is_settled_for_test()),
    );
    sender
        .unbounded_send(response("after-terminal", None))
        .expect("response to the retained platform");
    assert_eq!(platform.inner.run_foreground_task(), Some(0));
    app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        assert!(cx.windows().is_empty());
        let diagnostics = cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default());
        let responses = diagnostics
            .terminal
            .iter()
            .filter(|entry| entry.kind == response_kind)
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[2].disposition, NativeBoundaryDisposition::Closed);
    });
    assert!(callback_lifetime.upgrade().is_some());
    sender
        .unbounded_send(response("after-app-release", Some("open")))
        .expect("a queued transport response must not retain the App");
    drop(app);
    let released = weak_app
        .update_window(AnyWindowHandle::from(handle), |_, _, _| ())
        .expect_err("notification callbacks must not retain the App through its weak context");
    assert_eq!(released.to_string(), "app was released");
    assert!(callback_lifetime.upgrade().is_none());
    assert_eq!(platform.inner.run_foreground_task(), Some(0));
    assert_eq!(events.borrow().as_slice(), expected_events.as_slice());
    assert!(root.upgrade().is_none());
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(!unsafe { IsWindow(Some(registration.as_raw())).as_bool() });
    drop(weak_app);
    drop(platform);
    pump_messages_until_with_delay(
        "notification platform HWND, callback, App and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && platform_inner_lifetime.upgrade().is_none()
        },
    );
    assert!(sender.is_closed());
    assert!(sender.unbounded_send(response("retired", None)).is_err());
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
}
