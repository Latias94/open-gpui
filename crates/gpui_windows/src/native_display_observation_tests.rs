//! Bounded display-observation consumers over the actual Windows backend.
//!
//! Posted WM_DISPLAYCHANGE enters the real platform WndProc and rereads the current monitors.
//! It must not invent a topology or refresh-mode change. The association fixture below changes
//! only a detached Rust window binding; the separate TestAppContext fixture qualifies property
//! notifications and cache readiness. Neither fixture changes the user's physical display modes.
//! These tests do not qualify OS hotplug, mode switching, or compositor presentation.

use super::*;
use crate::{
    WM_GPUI_DISPLAYS_CHANGED, WM_GPUI_REFRESH_DISPLAY_TOPOLOGY, WindowsDisplayTopologyUnavailable,
    native_test_observation::NativeWindowTestMessageDisposition, with_windows_callback,
};
use open_gpui::{
    DisplayChanges, DisplayEvent, DisplayId, PlatformDisplay as _, TestAppContext, WindowHandle,
    WindowInitialPresentationStatus,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetParent, WM_DISPLAYCHANGE, WS_CHILD,
};

struct NativeDisplayRoot;

impl Render for NativeDisplayRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(white())
    }
}

/// Dispatches one already-posted message for the exact HWND without running foreground tasks.
/// This permits native ingress while the App borrow remains deliberately busy.
fn dispatch_display_message(hwnd: HWND, expected: u32) {
    let mut message = MSG::default();
    assert!(
        unsafe { PeekMessageW(&mut message, Some(hwnd), expected, expected, PM_REMOVE).as_bool() },
        "display message {expected:#x} must be queued for the exact native recipient"
    );
    assert_eq!(message.hwnd, hwnd);
    assert_eq!(message.message, expected);
    assert_eq!(unsafe { DispatchMessageW(&message) }, LRESULT(0));
}

fn reread_native_displays(platform: &WindowsPlatform, recipient: HWND) -> u64 {
    post_message(
        recipient,
        WM_DISPLAYCHANGE,
        WPARAM::default(),
        LPARAM::default(),
    );
    dispatch_display_message(recipient, WM_DISPLAYCHANGE);
    let Err(WindowsDisplayTopologyUnavailable::RefreshPending { request_epoch }) =
        platform.inner.exact_display_topology_snapshot()
    else {
        panic!("the real WM_DISPLAYCHANGE must invalidate exact topology before its queued reread");
    };
    dispatch_display_message(platform.handle, WM_GPUI_REFRESH_DISPLAY_TOPOLOGY);
    assert!(
        platform.inner.exact_display_topology_snapshot().is_ok(),
        "the queued native reread must restore a complete topology"
    );
    dispatch_display_message(platform.handle, WM_GPUI_DISPLAYS_CHANGED);
    request_epoch
}

fn wait_for_display_window_submission(
    app: &mut Application,
    handle: WindowHandle<NativeDisplayRoot>,
) {
    pump_messages_until_with_delay(
        "display-observation consumer's nonempty native Submitted frame",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| {
                handle.update(cx, |_, window, _| {
                    let facts = window.presentation_facts();
                    let Some(attempt) = facts.latest_present_attempt else {
                        return false;
                    };
                    facts.initial_presentation == WindowInitialPresentationStatus::Completed
                        && facts.native_visible
                        && attempt.generation > 0
                        && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                        && attempt.contained_valid_primitives
                        && facts.frame_accepted_generation == Some(attempt.generation)
                        && facts.present_submitted_generation == Some(attempt.generation)
                        && facts.non_empty_presented_generation == Some(attempt.generation)
                        && window.rendered_frame_revision() == attempt.generation
                        && window.submitted_frame_diagnostics_for_test().is_some_and(
                            |diagnostics| diagnostics.generation() == attempt.generation,
                        )
                })
            })
            .expect("the actual display-observation window must survive through submission")
        },
    );
}

#[test]
fn native_display_observation_rereads_without_windows_and_retires_exact_window_callbacks() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new(false)
            .expect("the native display-observation backend must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    assert!(unsafe { IsWindow(Some(platform_hwnd)).as_bool() });
    assert!(!unsafe { IsWindowVisible(platform_hwnd).as_bool() });
    assert_eq!(
        unsafe { GetAncestor(platform_hwnd, GA_ROOT) },
        platform_hwnd
    );
    assert!(
        unsafe { GetParent(platform_hwnd) }.is_err(),
        "the hidden platform HWND must have no parent, including HWND_MESSAGE"
    );
    let style = WINDOW_STYLE(unsafe { get_window_long(platform_hwnd, GWL_STYLE) } as u32);
    assert_eq!(style.0 & (WS_CHILD | WS_VISIBLE).0, 0);

    let initial_snapshot = platform.display_snapshot();
    let initial_generation = initial_snapshot
        .generation()
        .expect("native Windows display publications must have a generation");
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let global_text_lifetime = Arc::downgrade(&app.text_system());
    let app_events = Rc::new(RefCell::new(Vec::new()));
    let app_subscription = app.update_for_test(|cx| {
        assert!(cx.windows().is_empty());
        assert!(platform.raw_window_handles.read().is_empty());
        let app_events = app_events.clone();
        cx.observe_displays(move |event, _| app_events.borrow_mut().push(event))
    });
    let zero_window_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let first_request = reread_native_displays(&platform, platform_hwnd);
    pump_messages_until_idle("zero-window display ingress delivery");
    assert!(first_request > 0);
    assert_eq!(
        platform.display_snapshot().generation(),
        Some(initial_generation)
    );
    let no_window_diagnostics =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(zero_window_cursor));
    assert_eq!(no_window_diagnostics.omitted_before_cursor, 0);
    assert!(no_window_diagnostics.pending.is_empty());
    let app_ingress = no_window_diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Application
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::DisplaysChanged)
        })
        .collect::<Vec<_>>();
    assert_eq!(app_ingress.len(), 1);
    assert_eq!(
        app_ingress[0].disposition,
        NativeBoundaryDisposition::Delivered { input_result: None }
    );
    assert!(app_events.borrow().is_empty());
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());

    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(240.), px(160.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| NativeDisplayRoot),
            )
        })
        .expect("the display consumer must open a real Windows window");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("the display consumer must register its exact native HWND");
    let native_window = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the display consumer must have a real native owner");
    assert_eq!(native_window.registration, registration);
    let native_lifetime = Rc::downgrade(&native_window);
    let (root_lifetime, window_text_lifetime) = app.update_for_test(|cx| {
        let root = handle.root(cx).expect("the display root must be live");
        let window_text_lifetime = handle
            .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
            .expect("the display window must own a text cache");
        (root.downgrade(), window_text_lifetime)
    });
    platform.begin_vsync_thread();
    wait_for_display_window_submission(&mut app, handle);
    pump_messages_until_idle("display consumer initial native follow-up");
    let original_binding = native_window.state.display.get();
    let original_refresh = original_binding.refresh_interval();
    let original_generation = native_window.state.display_topology_generation.get();
    let window_observations = Rc::new(RefCell::new(Vec::new()));
    let window_subscription = app
        .update_for_test(|cx| {
            handle.update(cx, |_, window, cx| {
                assert_eq!(window.refresh_interval(cx), original_refresh);
                let observations = window_observations.clone();
                window.observe_window_display(move |window, cx| {
                    let display = window.display(cx);
                    let refresh = window.refresh_interval(cx);
                    assert_eq!(
                        refresh,
                        display.as_ref().and_then(|value| value.refresh_interval())
                    );
                    observations
                        .borrow_mut()
                        .push((display.map(|value| value.id()), refresh));
                })
            })
        })
        .expect("the real window must accept a display observer");

    let second_request = reread_native_displays(&platform, registration.as_raw());
    assert!(second_request > first_request);
    assert_eq!(
        platform.display_snapshot().generation(),
        Some(initial_generation)
    );
    assert!(
        with_windows_callback(&native_window.state.callbacks.display_changed, |callback| {
            callback()
        })
        .is_some()
    );
    pump_messages_until_idle("spurious display ingress delivery");
    assert!(app_events.borrow().is_empty());
    assert!(window_observations.borrow().is_empty());

    let fixture_id = (0..=initial_snapshot.displays().len())
        .map(|id| DisplayId::new(id as u64))
        .find(|id| initial_snapshot.find_display(*id).is_none())
        .expect("the finite native display collection must leave a fixture identity available");
    let mut fixture_binding = original_binding;
    fixture_binding.display_id = fixture_id;
    let busy_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let (app_sequence, window_sequence, busy_request) = app.update_for_test(|cx| {
        let request = reread_native_displays(&platform, registration.as_raw());
        assert!(request > second_request);
        native_window.set_display_binding(fixture_binding, original_generation);
        assert!(window_observations.borrow().is_empty());
        assert!(app_events.borrow().is_empty());
        let diagnostics = cx.native_boundary_diagnostics(busy_cursor);
        assert_eq!(diagnostics.omitted_before_cursor, 0);
        let app_pending = diagnostics
            .pending
            .iter()
            .find(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Application
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::DisplaysChanged)
            })
            .expect("the actual platform callback must queue while the App borrow is busy");
        let window_pending = diagnostics
            .pending
            .iter()
            .find(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::DisplayChanged)
            })
            .expect("the fixture association callback must share the ordered App ingress");
        assert!(app_pending.sequence < window_pending.sequence);
        assert!(!diagnostics.terminal.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind,
                NativeBoundaryKind::Callback(
                    NativeCallbackKind::DisplaysChanged | NativeCallbackKind::DisplayChanged
                )
            )
        }));
        (app_pending.sequence, window_pending.sequence, request)
    });
    pump_messages_until(
        "busy-App display ingress and detached association fixture",
        || {
            app.update_for_test(|cx| {
                let diagnostics = cx.native_boundary_diagnostics(busy_cursor);
                [app_sequence, window_sequence].into_iter().all(|sequence| {
                    diagnostics.terminal.iter().any(|diagnostic| {
                        diagnostic.sequence == sequence
                            && diagnostic.disposition
                                == NativeBoundaryDisposition::Delivered { input_result: None }
                    })
                })
            })
        },
    );
    let busy_diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(busy_cursor));
    assert_eq!(busy_diagnostics.omitted_before_cursor, 0);
    let ordered = busy_diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| [app_sequence, window_sequence].contains(&diagnostic.sequence))
        .map(|diagnostic| diagnostic.sequence)
        .collect::<Vec<_>>();
    assert_eq!(ordered, [app_sequence, window_sequence]);
    assert!(busy_diagnostics.terminal.iter().all(|diagnostic| !matches!(
        diagnostic.disposition,
        NativeBoundaryDisposition::InvariantFailure(_)
    )));
    assert_eq!(window_observations.borrow().as_slice(), &[(None, None)]);
    native_window.set_display_binding(original_binding, original_generation);
    pump_messages_until("restored detached display binding", || {
        window_observations.borrow().len() == 2
    });
    assert_eq!(
        window_observations.borrow().as_slice(),
        &[
            (None, None),
            (Some(original_binding.display_id), original_refresh)
        ]
    );

    let stale_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let stale_sequence = app.update_for_test(|cx| {
        native_window.set_display_binding(fixture_binding, original_generation);
        let sequence = cx
            .native_boundary_diagnostics(stale_cursor)
            .pending
            .iter()
            .find(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::DisplayChanged)
            })
            .expect("a real window must own the queued fixture callback before logical close")
            .sequence;
        handle
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("the display consumer must close through its owning Window");
        sequence
    });
    pump_messages_until_with_delay(
        "display consumer exact HWND retirement and stale ingress settlement",
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
            }) && !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
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
    assert_eq!(window_observations.borrow().len(), 2);
    assert!(app_events.borrow().is_empty());
    let stale_diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(stale_cursor));
    assert_eq!(stale_diagnostics.omitted_before_cursor, 0);
    assert!(!stale_diagnostics.terminal.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::DisplayChanged)
            && matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Delivered { .. }
            )
    }));
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
        panic!("unexpected display consumer retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    drop(native_window);
    assert!(native_lifetime.upgrade().is_none());

    assert!(reread_native_displays(&platform, platform_hwnd) > busy_request);
    pump_messages_until_idle("post-retirement display ingress delivery");
    assert_eq!(
        platform.display_snapshot().generation(),
        Some(initial_generation)
    );
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(app_events.borrow().is_empty());
    assert_eq!(window_observations.borrow().len(), 2);
    assert!(unsafe { IsWindow(Some(platform_hwnd)).as_bool() });
    drop(window_subscription);
    drop(app_subscription);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "display consumer platform HWND, native backend, text and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && global_text_lifetime.upgrade().is_none()
                && backend_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}

#[test]
fn deterministic_display_fixture_publishes_cached_refresh_before_app_and_window_observers() {
    let mut cx = TestAppContext::single();
    cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let handle = cx.add_window(|_, _| NativeDisplayRoot);
    let display_id = handle
        .update(&mut cx, |_, window, cx| window.display(cx).unwrap().id())
        .unwrap();
    let expected_refresh = Some(Duration::from_secs(1) / 144);
    let window_notifications = Rc::new(Cell::new(0));
    let window_subscription = handle
        .update(&mut cx, {
            let notifications = window_notifications.clone();
            move |_, window, _| {
                window.observe_window_display(move |window, cx| {
                    assert_eq!(window.refresh_interval(cx), expected_refresh);
                    assert_eq!(window.display(cx).unwrap().id(), display_id);
                    notifications.set(notifications.get() + 1);
                })
            }
        })
        .unwrap();
    let app_events = Rc::new(RefCell::new(Vec::new()));
    let app_subscription = cx.update({
        let events = app_events.clone();
        move |cx| {
            cx.observe_displays(move |event, cx| {
                handle
                    .update(cx, |_, window, cx| {
                        assert_eq!(window.refresh_interval(cx), expected_refresh);
                    })
                    .unwrap();
                events.borrow_mut().push(event);
            })
        }
    });
    cx.simulate_display_refresh_interval_change(display_id, expected_refresh);
    cx.run_until_parked();
    assert_eq!(window_notifications.get(), 1);
    assert_eq!(
        app_events.borrow().as_slice(),
        &[DisplayEvent::Changed {
            id: display_id,
            changes: DisplayChanges::REFRESH_INTERVAL
        }]
    );
    cx.simulate_display_refresh_interval_change(display_id, expected_refresh);
    cx.simulate_displays_changed();
    cx.run_until_parked();
    assert_eq!(window_notifications.get(), 1);
    assert_eq!(app_events.borrow().len(), 1);
    drop(window_subscription);
    drop(app_subscription);
    handle
        .update(&mut cx, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}
