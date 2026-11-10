//! Sleep/wake and presentation-visibility consumers over the actual Windows backend.
//!
//! Power messages are synthetic and target the real hidden platform HWND. Visibility transitions
//! use actual native show, hide, minimize, and restore operations; unchanged message hints are
//! synthetic. These tests do not suspend the system, change display modes, or qualify compositor
//! presentation, full occlusion, or display sleep.

use super::*;
use crate::{
    native_test_observation::{
        NativeWindowTestMessageDisposition, NativeWindowTestObservation,
        NativeWindowTestObservationCursor,
    },
    window::window_from_hwnd,
};
use open_gpui::{
    App, NativeBoundaryDiagnosticsSnapshot, Subscription, WindowHandle,
    WindowInitialPresentationStatus, WindowVisibility,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetParent, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    SW_HIDE, SW_RESTORE, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, WM_POWERBROADCAST, WM_SHOWWINDOW,
    WM_WINDOWPOSCHANGED,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PowerVisibilityEvent {
    Sleep(usize),
    Wake(usize),
    Visibility(WindowId, WindowVisibility),
}

struct NativePowerVisibilityRoot;

impl Render for NativePowerVisibilityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(white())
    }
}

fn observe_native_power(
    cx: &App,
    events: &Rc<RefCell<Vec<PowerVisibilityEvent>>>,
) -> Vec<Subscription> {
    let mut subscriptions = Vec::new();
    for subscriber in 0..2 {
        let sleep_events = events.clone();
        subscriptions.push(cx.on_system_sleep(move |_| {
            sleep_events
                .borrow_mut()
                .push(PowerVisibilityEvent::Sleep(subscriber));
        }));
        let wake_events = events.clone();
        subscriptions.push(cx.on_system_wake(move |_| {
            wake_events
                .borrow_mut()
                .push(PowerVisibilityEvent::Wake(subscriber));
        }));
    }
    subscriptions
}

fn dispatch_native_power(hwnd: HWND, event: u32) {
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    post_message(hwnd, WM_POWERBROADCAST, WPARAM(event as usize), LPARAM(0));
    let mut message = MSG::default();
    assert!(unsafe {
        PeekMessageW(
            &mut message,
            Some(hwnd),
            WM_POWERBROADCAST,
            WM_POWERBROADCAST,
            PM_REMOVE,
        )
        .as_bool()
    });
    assert_eq!(message.hwnd, hwnd);
    assert_eq!(message.message, WM_POWERBROADCAST);
    assert_eq!(message.wParam, WPARAM(event as usize));
    assert_eq!(message.lParam, LPARAM(0));
    assert_eq!(unsafe { DispatchMessageW(&message) }, LRESULT(1));
}

fn power_visibility_diagnostics(
    diagnostics: &NativeBoundaryDiagnosticsSnapshot,
) -> Vec<(u64, NativeCallbackKind, NativeBoundaryDisposition)> {
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| !matches!(
        diagnostic.disposition,
        NativeBoundaryDisposition::InvariantFailure(_)
    )));
    diagnostics
        .terminal
        .iter()
        .filter_map(|diagnostic| {
            let NativeBoundaryKind::Callback(kind) = diagnostic.kind else {
                return None;
            };
            matches!(
                kind,
                NativeCallbackKind::SystemSleep
                    | NativeCallbackKind::SystemWake
                    | NativeCallbackKind::VisibilityChanged
            )
            .then_some((diagnostic.sequence, kind, diagnostic.disposition))
        })
        .collect()
}

fn assert_native_visibility_source(
    observation: &NativeWindowTestObservation,
    cursor: NativeWindowTestObservationCursor,
    registration: RegisteredWindow,
    message: u32,
    expected_wparam: usize,
) {
    let receipts = observation
        .events_after(cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .collect::<Vec<_>>();
    assert!(receipts.iter().all(|event| !matches!(
        event.kind(),
        NativeWindowTestEventKind::WindowMessage {
            disposition: NativeWindowTestMessageDisposition::Panicked,
            ..
        }
    )));
    assert!(
        receipts.iter().any(|event| matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::Other(observed),
                wparam,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if observed == message && wparam == expected_wparam
        )),
        "the exact HWND generation must receive native message {message:#x} with wparam {expected_wparam}"
    );
}

fn wait_for_native_visibility_submission(
    app: &mut Application,
    handle: WindowHandle<NativePowerVisibilityRoot>,
    after_generation: u64,
) -> u64 {
    let mut submitted_generation = None;
    pump_messages_until_with_delay(
        "visibility consumer's nonempty native Submitted frame",
        Duration::from_millis(5),
        || {
            submitted_generation = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        (window.visibility() == WindowVisibility::Visible
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && attempt.generation > after_generation
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && window.submitted_frame_diagnostics_for_test().is_some_and(
                                |diagnostics| diagnostics.generation() == attempt.generation,
                            ))
                        .then_some(attempt.generation)
                    })
                })
                .expect("the visibility consumer must survive through native submission");
            submitted_generation.is_some()
        },
    );
    submitted_generation.expect("native submission must publish its exact generation")
}

#[test]
fn native_system_sleep_wake_share_registration_and_preserve_busy_app_subscriptions() {
    discard_stale_quit_messages();
    let platform =
        Rc::new(WindowsPlatform::new(true).expect("the native power backend must initialize"));
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    assert!(!unsafe { IsWindowVisible(platform_hwnd).as_bool() });
    assert_eq!(
        unsafe { GetAncestor(platform_hwnd, GA_ROOT) },
        platform_hwnd
    );
    assert!(unsafe { GetParent(platform_hwnd) }.is_err());
    assert!(platform.suspend_resume_notification.borrow().is_none());

    let sleep_count = Arc::new(AtomicUsize::new(0));
    platform.on_system_sleep(Arc::new({
        let count = sleep_count.clone();
        move || {
            count.fetch_add(1, Ordering::Relaxed);
        }
    }));
    let notification = platform
        .suspend_resume_notification
        .borrow()
        .expect("sleep-only registration must subscribe the actual platform HWND");
    dispatch_native_power(platform_hwnd, PBT_APMSUSPEND);
    assert_eq!(sleep_count.load(Ordering::Relaxed), 1);

    let wake_count = Arc::new(AtomicUsize::new(0));
    platform.on_system_wake(Arc::new({
        let count = wake_count.clone();
        move || {
            count.fetch_add(1, Ordering::Relaxed);
        }
    }));
    assert_eq!(
        *platform.suspend_resume_notification.borrow(),
        Some(notification)
    );
    dispatch_native_power(platform_hwnd, PBT_APMRESUMEAUTOMATIC);
    dispatch_native_power(platform_hwnd, PBT_APMRESUMESUSPEND);
    assert_eq!(wake_count.load(Ordering::Relaxed), 1);

    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut subscriptions = app.update_for_test(|cx| observe_native_power(cx, &events));
    let cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let sequences = app.update_for_test(|cx| {
        assert!(cx.windows().is_empty());
        dispatch_native_power(platform_hwnd, PBT_APMSUSPEND);
        dispatch_native_power(platform_hwnd, PBT_APMRESUMEAUTOMATIC);
        dispatch_native_power(platform_hwnd, PBT_APMRESUMESUSPEND);
        assert!(events.borrow().is_empty());
        let diagnostics = cx.native_boundary_diagnostics(cursor);
        assert!(power_visibility_diagnostics(&diagnostics).is_empty());
        let pending = diagnostics
            .pending
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Application
                    && matches!(
                        diagnostic.kind,
                        NativeBoundaryKind::Callback(
                            NativeCallbackKind::SystemSleep | NativeCallbackKind::SystemWake
                        )
                    )
            })
            .map(|diagnostic| (diagnostic.sequence, diagnostic.kind))
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 2);
        assert!(pending[0].0 < pending[1].0);
        assert_eq!(
            pending[0].1,
            NativeBoundaryKind::Callback(NativeCallbackKind::SystemSleep)
        );
        assert_eq!(
            pending[1].1,
            NativeBoundaryKind::Callback(NativeCallbackKind::SystemWake)
        );
        [pending[0].0, pending[1].0]
    });
    pump_messages_until("busy-App native sleep/wake delivery", || {
        events.borrow().len() == 4
    });
    assert_eq!(
        events.borrow().as_slice(),
        &[
            PowerVisibilityEvent::Sleep(0),
            PowerVisibilityEvent::Sleep(1),
            PowerVisibilityEvent::Wake(0),
            PowerVisibilityEvent::Wake(1),
        ]
    );
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(cursor));
    assert_eq!(
        power_visibility_diagnostics(&diagnostics),
        [
            (
                sequences[0],
                NativeCallbackKind::SystemSleep,
                NativeBoundaryDisposition::Delivered { input_result: None },
            ),
            (
                sequences[1],
                NativeCallbackKind::SystemWake,
                NativeBoundaryDisposition::Delivered { input_result: None },
            ),
        ]
    );
    subscriptions.truncate(2);
    dispatch_native_power(platform_hwnd, PBT_APMSUSPEND);
    dispatch_native_power(platform_hwnd, PBT_APMRESUMEAUTOMATIC);
    pump_messages_until("remaining native power subscriptions", || {
        events.borrow().len() == 6
    });
    assert_eq!(
        &events.borrow()[4..],
        &[
            PowerVisibilityEvent::Sleep(0),
            PowerVisibilityEvent::Wake(0)
        ]
    );
    drop(subscriptions);
    drop(app);

    let mut rebuilt =
        Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let rebuilt_events = Rc::new(RefCell::new(Vec::new()));
    let rebuilt_subscriptions =
        rebuilt.update_for_test(|cx| observe_native_power(cx, &rebuilt_events));
    dispatch_native_power(platform_hwnd, PBT_APMSUSPEND);
    dispatch_native_power(platform_hwnd, PBT_APMRESUMEAUTOMATIC);
    pump_messages_until("reconstructed App native power subscriptions", || {
        rebuilt_events.borrow().len() == 4
    });
    assert_eq!(events.borrow().len(), 6);
    assert_eq!(
        *platform.suspend_resume_notification.borrow(),
        Some(notification)
    );
    assert!(platform.raw_window_handles.read().is_empty());
    drop(rebuilt_subscriptions);
    drop(rebuilt);
    drop(platform);
    pump_messages_until_with_delay(
        "native power platform retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && lifecycle_probe.ole_uninitialize_count() == 1
        },
    );
    assert_eq!(lifecycle_probe.suspend_resume_unregister_attempts(), 1);
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);

    let wake_first = WindowsPlatform::new(true).expect("wake-first backend must initialize");
    wake_first.on_system_wake(Arc::new(|| {}));
    let notification = *wake_first.suspend_resume_notification.borrow();
    assert!(notification.is_some());
    wake_first.on_system_sleep(Arc::new(|| {}));
    assert_eq!(
        *wake_first.suspend_resume_notification.borrow(),
        notification
    );
    drop(wake_first);
}

#[test]
fn native_visibility_tracks_presentation_and_retires_exact_hwnd_callbacks() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new_windowed_for_test()
            .expect("the native visibility backend must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let global_text_lifetime = Arc::downgrade(&app.text_system());
    let events = Rc::new(RefCell::new(Vec::new()));
    let power_subscriptions = app.update_for_test(|cx| observe_native_power(cx, &events));
    let mut visibility_subscription = None;
    let initial_source = observation.cursor();
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(240.), px(160.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    assert_eq!(window.visibility(), WindowVisibility::Hidden);
                    assert!(!window.is_visible());
                    assert!(!window.presentation_facts().native_visible);
                    let window_id = window.window_handle().window_id();
                    let events = events.clone();
                    visibility_subscription = Some(window.observe_window_visibility(
                        move |visibility, window, _| {
                            assert_eq!(window.visibility(), visibility);
                            assert_eq!(window.is_visible(), visibility.is_visible());
                            events
                                .borrow_mut()
                                .push(PowerVisibilityEvent::Visibility(window_id, visibility));
                        },
                    ));
                    cx.new(|_| NativePowerVisibilityRoot)
                },
            )
        })
        .expect("the visibility consumer must complete its initially hidden native presentation");
    let visibility_subscription =
        visibility_subscription.expect("the root builder must register its visibility observer");
    let registration = *platform
        .raw_window_handles
        .read()
        .iter()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("the visibility consumer must register its exact HWND");
    let native_window = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the visibility consumer must own its native HWND");
    let native_lifetime = Rc::downgrade(&native_window);
    let (root_lifetime, window_text_lifetime) = app.update_for_test(|cx| {
        let root = handle
            .root(cx)
            .expect("the visibility root must exist")
            .downgrade();
        let text = handle
            .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
            .expect("the visibility consumer must retain its window text system");
        (root, text)
    });
    assert_eq!(native_window.visibility(), WindowVisibility::Visible);
    assert!(
        native_window
            .initial_presentation_window_pos_flags_for_test()
            .iter()
            .any(|flags| flags.contains(SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE))
    );
    assert_native_visibility_source(
        &observation,
        initial_source,
        registration,
        WM_WINDOWPOSCHANGED,
        0,
    );
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");

    pump_messages_until("native initial visibility", || events.borrow().len() == 1);
    let initial_submission = wait_for_native_visibility_submission(&mut app, handle, 0);
    pump_messages_until_idle("initial native visibility reports");

    let source = observation.cursor();
    unsafe {
        SetWindowPos(
            registration.as_raw(),
            None,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW
                | SWP_NOACTIVATE
                | SWP_NOOWNERZORDER
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER,
        )
    }
    .expect("SetWindowPos must hide the real visibility consumer without changing its geometry");
    assert_native_visibility_source(&observation, source, registration, WM_WINDOWPOSCHANGED, 0);
    assert_eq!(native_window.visibility(), WindowVisibility::Hidden);
    pump_messages_until("native hidden visibility", || events.borrow().len() == 2);
    pump_messages_until_idle("hidden SetWindowPos visibility reports");

    let source = observation.cursor();
    unsafe {
        SetWindowPos(
            registration.as_raw(),
            None,
            0,
            0,
            0,
            0,
            SWP_SHOWWINDOW
                | SWP_NOACTIVATE
                | SWP_NOOWNERZORDER
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER,
        )
    }
    .expect("SetWindowPos must show the real visibility consumer without changing its geometry");
    assert_native_visibility_source(&observation, source, registration, WM_WINDOWPOSCHANGED, 0);
    assert_eq!(native_window.visibility(), WindowVisibility::Visible);
    pump_messages_until("native shown visibility", || events.borrow().len() == 3);

    let source = observation.cursor();
    let _ = unsafe { ShowWindow(registration.as_raw(), SW_SHOWMINNOACTIVE) };
    assert_native_visibility_source(
        &observation,
        source,
        registration,
        WM_SIZE,
        SIZE_MINIMIZED as usize,
    );
    assert!(unsafe { IsWindowVisible(registration.as_raw()).as_bool() });
    assert!(unsafe { IsIconic(registration.as_raw()).as_bool() });
    assert_eq!(native_window.visibility(), WindowVisibility::Hidden);
    pump_messages_until("native minimized presentation visibility", || {
        events.borrow().len() == 4
    });
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert!(window.presentation_facts().native_visible);
            assert!(window.is_minimized());
            assert!(!window.is_visible());
        })
    })
    .expect("the minimized consumer must retain its shown/mapped native facts");

    let source = observation.cursor();
    let _ = unsafe { ShowWindow(registration.as_raw(), SW_RESTORE) };
    assert_native_visibility_source(
        &observation,
        source,
        registration,
        WM_SIZE,
        SIZE_RESTORED as usize,
    );
    pump_messages_until("native restored presentation visibility", || {
        events.borrow().len() == 5
    });
    let restored_revision = app
        .update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                assert!(!window.is_minimized());
                let revision = window.rendered_frame_revision();
                window.refresh();
                revision
            })
        })
        .expect("the restored window must accept a fresh render");
    assert!(restored_revision >= initial_submission);
    wait_for_native_visibility_submission(&mut app, handle, restored_revision);
    assert_eq!(
        events.borrow().as_slice(),
        &[
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Visible),
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Hidden),
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Visible),
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Hidden),
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Visible),
        ]
    );

    let unchanged_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let mut client = RECT::default();
    unsafe { GetClientRect(registration.as_raw(), &mut client) }
        .expect("unchanged size hints must carry the actual native client size");
    let size_hint =
        LPARAM(((client.bottom as u32 & 0xffff) << 16 | (client.right as u32 & 0xffff)) as isize);
    let source = observation.cursor();
    let _ = unsafe {
        SendMessageW(
            registration.as_raw(),
            WM_SHOWWINDOW,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        )
    };
    let _ = unsafe {
        SendMessageW(
            registration.as_raw(),
            WM_SIZE,
            Some(WPARAM(SIZE_RESTORED as usize)),
            Some(size_hint),
        )
    };
    assert_native_visibility_source(&observation, source, registration, WM_SHOWWINDOW, 0);
    assert_native_visibility_source(
        &observation,
        source,
        registration,
        WM_SIZE,
        SIZE_RESTORED as usize,
    );
    pump_messages_until_idle("unchanged native visibility hints");
    assert_eq!(events.borrow().len(), 5);
    let unchanged = app.update_for_test(|cx| cx.native_boundary_diagnostics(unchanged_cursor));
    assert!(power_visibility_diagnostics(&unchanged).is_empty());

    let busy_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let sequences = app.update_for_test(|cx| {
        dispatch_native_power(platform_hwnd, PBT_APMSUSPEND);
        let _ = unsafe { ShowWindow(registration.as_raw(), SW_HIDE) };
        platform.inner.run_foreground_task();
        dispatch_native_power(platform_hwnd, PBT_APMRESUMEAUTOMATIC);
        assert_eq!(events.borrow().len(), 5);
        let diagnostics = cx.native_boundary_diagnostics(busy_cursor);
        assert!(power_visibility_diagnostics(&diagnostics).is_empty());
        let mut pending = diagnostics
            .pending
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.kind,
                    NativeBoundaryKind::Callback(
                        NativeCallbackKind::SystemSleep
                            | NativeCallbackKind::VisibilityChanged
                            | NativeCallbackKind::SystemWake
                    )
                )
            })
            .map(|diagnostic| (diagnostic.sequence, diagnostic.kind, diagnostic.target))
            .collect::<Vec<_>>();
        pending.sort_by_key(|entry| entry.0);
        assert_eq!(pending.len(), 3);
        assert_eq!(
            pending[0].1,
            NativeBoundaryKind::Callback(NativeCallbackKind::SystemSleep)
        );
        assert_eq!(
            pending[1].1,
            NativeBoundaryKind::Callback(NativeCallbackKind::VisibilityChanged)
        );
        assert_eq!(
            pending[1].2,
            NativeBoundaryTarget::Window(handle.window_id())
        );
        assert_eq!(
            pending[2].1,
            NativeBoundaryKind::Callback(NativeCallbackKind::SystemWake)
        );
        [pending[0].0, pending[1].0, pending[2].0]
    });
    pump_messages_until(
        "ordered busy-App native power and visibility delivery",
        || events.borrow().len() == 10,
    );
    assert_eq!(
        &events.borrow()[5..],
        &[
            PowerVisibilityEvent::Sleep(0),
            PowerVisibilityEvent::Sleep(1),
            PowerVisibilityEvent::Visibility(handle.window_id(), WindowVisibility::Hidden),
            PowerVisibilityEvent::Wake(0),
            PowerVisibilityEvent::Wake(1),
        ]
    );
    let busy = app.update_for_test(|cx| cx.native_boundary_diagnostics(busy_cursor));
    let delivered = power_visibility_diagnostics(&busy);
    assert_eq!(
        delivered.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        sequences
    );
    assert!(
        delivered.iter().all(|entry| {
            entry.2 == NativeBoundaryDisposition::Delivered { input_result: None }
        })
    );

    let _ = unsafe { ShowWindow(registration.as_raw(), SW_SHOWNOACTIVATE) };
    pump_messages_until("visibility before native teardown", || {
        events.borrow().len() == 11
    });
    let stale_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let stale_sequence = app.update_for_test(|cx| {
        let _ = unsafe { ShowWindow(registration.as_raw(), SW_HIDE) };
        platform.inner.run_foreground_task();
        let sequence = cx
            .native_boundary_diagnostics(stale_cursor)
            .pending
            .iter()
            .find(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::VisibilityChanged)
            })
            .expect("native visibility must be queued before logical close")
            .sequence;
        handle
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("the visibility consumer must close through its owning Window");
        sequence
    });
    pump_messages_until_with_delay(
        "exact visibility HWND retirement and stale ingress",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| {
                cx.windows().is_empty()
                    && cx
                        .native_boundary_diagnostics(stale_cursor)
                        .terminal
                        .iter()
                        .any(|diagnostic| {
                            diagnostic.sequence == stale_sequence
                                && matches!(
                                    diagnostic.disposition,
                                    NativeBoundaryDisposition::Stale
                                        | NativeBoundaryDisposition::Closed
                                )
                        })
            }) && window_from_hwnd(registration.as_raw()).is_none()
                && !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform
                    .raw_window_handles
                    .read()
                    .iter()
                    .any(|entry| entry.matches(registration))
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
                && window_text_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(events.borrow().len(), 11);
    assert_eq!(native_window.state.last_visibility.get(), None);
    assert!(
        native_window
            .state
            .callbacks
            .visibility_change
            .take()
            .is_none()
    );

    assert_eq!(native_window.visibility(), WindowVisibility::Hidden);
    native_window.report_visibility();
    let mut rebuilt_subscription = None;
    let source = observation.cursor();
    let rebuilt = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(240.), px(160.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    assert_eq!(window.visibility(), WindowVisibility::Hidden);
                    assert!(!window.presentation_facts().native_visible);
                    let window_id = window.window_handle().window_id();
                    let events = events.clone();
                    rebuilt_subscription = Some(window.observe_window_visibility(
                        move |visibility, window, _| {
                            assert_eq!(window.visibility(), visibility);
                            events
                                .borrow_mut()
                                .push(PowerVisibilityEvent::Visibility(window_id, visibility));
                        },
                    ));
                    cx.new(|_| NativePowerVisibilityRoot)
                },
            )
        })
        .expect("a visibility consumer must reconstruct after exact native retirement");
    let rebuilt_subscription = rebuilt_subscription
        .expect("the reconstructed root builder must register its visibility observer");
    let rebuilt_registration = *platform
        .raw_window_handles
        .read()
        .iter()
        .find(|entry| entry.window_id() == rebuilt.window_id())
        .expect("the reconstructed window must register a fresh native generation");
    assert_ne!(rebuilt.window_id(), handle.window_id());
    assert_ne!(rebuilt_registration.generation(), registration.generation());
    let rebuilt_native = platform
        .window_from_hwnd(rebuilt_registration.as_raw())
        .expect("the reconstructed window must own its exact HWND");
    let rebuilt_lifetime = Rc::downgrade(&rebuilt_native);
    assert_native_visibility_source(
        &observation,
        source,
        rebuilt_registration,
        WM_WINDOWPOSCHANGED,
        0,
    );
    pump_messages_until("reconstructed native visibility", || {
        events.borrow().len() == 12
    });
    assert_eq!(
        events.borrow()[11],
        PowerVisibilityEvent::Visibility(rebuilt.window_id(), WindowVisibility::Visible)
    );
    wait_for_native_visibility_submission(&mut app, rebuilt, 0);
    app.update_for_test(|cx| {
        let _ = unsafe { ShowWindow(rebuilt_registration.as_raw(), SW_HIDE) };
        rebuilt
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("a consumer with a pending native visibility report must close");
    });
    drop(rebuilt_native);
    drop(native_window);
    pump_messages_until_with_delay(
        "pending visibility report must not retain retired owners",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| cx.windows().is_empty())
                && native_lifetime.upgrade().is_none()
                && rebuilt_lifetime.upgrade().is_none()
                && !unsafe { IsWindow(Some(rebuilt_registration.as_raw())).as_bool() }
                && platform.raw_window_handles.read().is_empty()
                && platform.inner.native_retirement_is_settled()
        },
    );
    assert_eq!(events.borrow().len(), 12);
    let stale = app.update_for_test(|cx| cx.native_boundary_diagnostics(stale_cursor));
    assert!(!stale.terminal.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
            && diagnostic.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::VisibilityChanged)
            && matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Delivered { .. }
            )
    }));
    assert!(observation.events().iter().all(|event| !matches!(
        event.kind(),
        NativeWindowTestEventKind::WindowMessage {
            disposition: NativeWindowTestMessageDisposition::Panicked,
            ..
        }
    )));
    drop(rebuilt_subscription);
    drop(visibility_subscription);
    drop(power_subscriptions);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "visibility backend and text authority release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && backend_lifetime.upgrade().is_none()
                && global_text_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.suspend_resume_unregister_attempts(), 1);
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
