use super::{WindowsPlatform, translate_accelerator};
use crate::{WindowsWindowInner, get_window_long};
use open_gpui::{
    AnyWindowHandle, AppContext as _, Application, Empty, NativeBoundaryDiagnosticCursor,
    NativeBoundaryDisposition, NativeBoundaryKind, NativeBoundaryTarget, NativeCallbackKind,
    NativePlatformCommandKind, PlatformInput, PointerCancelEvent, PointerCancelReason, QuitMode,
    WindowActivationPolicy, WindowBounds, WindowKind, WindowMouseEvent, WindowMutationDispatch,
    WindowMutationOutcome, WindowOptions, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Gdi::{RDW_INVALIDATE, RedrawWindow},
    System::SystemServices::MK_LBUTTON,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{
            GetActiveWindow, GetCapture, IsWindowEnabled, ReleaseCapture, SetActiveWindow,
        },
        WindowsAndMessaging::{
            DispatchMessageW, GW_HWNDFIRST, GW_HWNDNEXT, GW_OWNER, GWL_EXSTYLE,
            GetForegroundWindow, GetWindow, GetWindowRect, IsWindow, IsWindowVisible, IsZoomed,
            MA_NOACTIVATE, MSG, PM_REMOVE, PeekMessageW, PostMessageW, SIZE_MINIMIZED,
            SIZE_RESTORED, SendMessageW, TranslateMessage, WM_CLOSE, WM_KEYDOWN, WM_KEYUP,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOVE, WM_PAINT,
            WM_QUIT, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WS_EX_NOACTIVATE,
        },
    },
};

const MAX_MESSAGE_PUMP_ATTEMPTS: usize = 512;
const MAX_STALE_QUIT_MESSAGES: usize = 16;

fn native_input_delivery_count(app: &mut open_gpui::App, propagate: bool) -> usize {
    app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .terminal
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                && matches!(
                    diagnostic.disposition,
                    NativeBoundaryDisposition::Delivered {
                        input_result: Some(result),
                    } if result.propagate == propagate
                )
        })
        .count()
}

#[derive(Clone, Copy)]
struct DispatchedMessage {
    hwnd: HWND,
    message: u32,
    result: isize,
}

#[derive(Default)]
struct MessageTrace {
    dispatched: Vec<DispatchedMessage>,
}

impl MessageTrace {
    fn contains(&self, hwnd: HWND, message: u32) -> bool {
        self.dispatched
            .iter()
            .any(|record| record.hwnd == hwnd && record.message == message)
    }

    fn last_result(&self, hwnd: HWND, message: u32) -> Option<isize> {
        self.dispatched
            .iter()
            .rev()
            .find(|record| record.hwnd == hwnd && record.message == message)
            .map(|record| record.result)
    }

    fn message_ids(&self) -> Vec<u32> {
        self.dispatched
            .iter()
            .map(|record| record.message)
            .collect()
    }
}

fn discard_stale_quit_messages() {
    for _ in 0..MAX_STALE_QUIT_MESSAGES {
        let mut message = MSG::default();
        if !unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() } {
            return;
        }
    }

    panic!("native test thread retained too many stale WM_QUIT messages");
}

fn dispatch_one_message(trace: &mut MessageTrace) -> bool {
    let mut message = MSG::default();
    if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
        return false;
    }

    assert_ne!(
        message.message, WM_QUIT,
        "explicit-quit native test unexpectedly received WM_QUIT"
    );

    if translate_accelerator(&message).is_none() {
        _ = unsafe { TranslateMessage(&message) };
        let result = unsafe { DispatchMessageW(&message) };
        trace.dispatched.push(DispatchedMessage {
            hwnd: message.hwnd,
            message: message.message,
            result: result.0,
        });
    }

    true
}

fn pump_messages_until(description: &str, mut converged: impl FnMut() -> bool) -> MessageTrace {
    let mut trace = MessageTrace::default();
    for _ in 0..MAX_MESSAGE_PUMP_ATTEMPTS {
        if converged() {
            return trace;
        }
        dispatch_one_message(&mut trace);
    }

    if converged() {
        return trace;
    }

    panic!(
        "{description} did not converge within {MAX_MESSAGE_PUMP_ATTEMPTS} message-pump attempts; dispatched={:x?}",
        trace.message_ids()
    );
}

fn pump_messages_until_idle(description: &str) -> MessageTrace {
    let mut trace = MessageTrace::default();
    for _ in 0..MAX_MESSAGE_PUMP_ATTEMPTS {
        if !dispatch_one_message(&mut trace) {
            return trace;
        }
    }

    panic!(
        "{description} did not become idle within {MAX_MESSAGE_PUMP_ATTEMPTS} dispatched messages; dispatched={:x?}",
        trace.message_ids()
    );
}

fn post_message(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    unsafe { PostMessageW(Some(hwnd), message, wparam, lparam) }
        .unwrap_or_else(|error| panic!("failed to post native test message {message:#x}: {error}"));
}

fn mouse_position_lparam(x: u16, y: u16) -> LPARAM {
    LPARAM(((u32::from(y) << 16) | u32::from(x)) as isize)
}

fn observe_frame_requests(window: &Rc<WindowsWindowInner>) -> Rc<Cell<usize>> {
    let frame_requests = Rc::new(Cell::new(0usize));
    let mut callback = window
        .state
        .callbacks
        .request_frame
        .take()
        .expect("native test window should have a frame callback");
    window.state.callbacks.request_frame.set(Some(Box::new({
        let frame_requests = frame_requests.clone();
        move |options| {
            callback(options);
            frame_requests.set(frame_requests.get().saturating_add(1));
        }
    })));
    frame_requests
}

fn is_registered(platform: &WindowsPlatform, hwnd: HWND) -> bool {
    platform
        .raw_window_handles
        .read()
        .iter()
        .any(|handle| handle.as_raw() == hwnd)
}

fn application_window_z_order(platform: &WindowsPlatform) -> Vec<HWND> {
    let registered = platform
        .raw_window_handles
        .read()
        .iter()
        .map(|handle| handle.as_raw())
        .collect::<Vec<_>>();
    let Some(seed) = registered.first().copied() else {
        return Vec::new();
    };
    let mut current = unsafe { GetWindow(seed, GW_HWNDFIRST) }
        .expect("a registered top-level window should belong to the desktop z-order");
    let mut ordered = Vec::with_capacity(registered.len());

    for _ in 0..4096 {
        if registered.contains(&current) {
            ordered.push(current);
            if ordered.len() == registered.len() {
                return ordered;
            }
        }
        let Ok(next) = (unsafe { GetWindow(current, GW_HWNDNEXT) }) else {
            break;
        };
        current = next;
    }

    panic!(
        "desktop z-order walk did not observe every registered application window; registered={registered:?}, observed={ordered:?}"
    );
}

fn native_window_rect(hwnd: HWND) -> (i32, i32, i32, i32) {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.expect("native window rect should be readable");
    (rect.left, rect.top, rect.right, rect.bottom)
}

#[test]
fn real_hwnd_lifecycle_and_input_dispatch_converge_with_bounded_message_pump() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();

    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(unsafe { IsWindowVisible(hwnd).as_bool() });
    assert!(is_registered(&platform, hwnd));
    assert_eq!(
        platform.lifecycle_test_probe.hidden_before_map(),
        Some(true),
        "WindowsWindow construction must remain hidden until map_window"
    );

    let consume_mouse_move = Rc::new(Cell::new(false));
    let _mouse_interceptor = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let consume_mouse_move = consume_mouse_move.clone();
                    move |event, window, cx| {
                        if consume_mouse_move.get() && matches!(event, WindowMouseEvent::Move(_)) {
                            cx.stop_propagation();
                            window.prevent_default();
                        }
                    }
                })
            })
        })
        .expect("native test window should accept an input interceptor");
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let registered_window = native_window.registration;
    let frame_requests = observe_frame_requests(&native_window);

    let frame_request_baseline = frame_requests.get();
    assert!(
        unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() },
        "native test window should accept paint invalidation"
    );
    let paint_trace = pump_messages_until("native WM_PAINT", || {
        frame_requests.get() > frame_request_baseline
    });
    assert!(
        paint_trace.contains(hwnd, WM_PAINT),
        "frame callback must originate from a dispatched WM_PAINT"
    );
    pump_messages_until_idle("native paint follow-up");

    let propagated_before = app.update_for_test(|cx| native_input_delivery_count(cx, true));
    post_message(
        hwnd,
        WM_MOUSEMOVE,
        WPARAM::default(),
        mouse_position_lparam(12, 18),
    );
    let propagated_trace = pump_messages_until("propagated native mouse input", || {
        app.update_for_test(|cx| native_input_delivery_count(cx, true)) > propagated_before
    });
    assert_eq!(
        propagated_trace.last_result(hwnd, WM_MOUSEMOVE),
        Some(1),
        "Win32 must receive the propagated input disposition"
    );
    pump_messages_until_idle("propagated input follow-up");

    consume_mouse_move.set(true);
    let consumed_before = app.update_for_test(|cx| native_input_delivery_count(cx, false));
    post_message(
        hwnd,
        WM_MOUSEMOVE,
        WPARAM::default(),
        mouse_position_lparam(24, 30),
    );
    let consumed_trace = pump_messages_until("consumed native mouse input", || {
        app.update_for_test(|cx| native_input_delivery_count(cx, false)) > consumed_before
    });
    assert_eq!(
        consumed_trace.last_result(hwnd, WM_MOUSEMOVE),
        Some(0),
        "Win32 must receive the consumed input disposition"
    );
    pump_messages_until_idle("consumed input follow-up");

    let any_window = AnyWindowHandle::from(window);
    post_message(hwnd, WM_CLOSE, WPARAM::default(), LPARAM::default());
    let close_trace = pump_messages_until("native WM_CLOSE", || {
        let app_window_closed =
            app.update_for_test(|cx| any_window.update(cx, |_, _, _| ()).is_err());
        !unsafe { IsWindow(Some(hwnd)).as_bool() }
            && !is_registered(&platform, hwnd)
            && app_window_closed
    });
    assert!(close_trace.contains(hwnd, WM_CLOSE));
    platform.raw_window_handles.write().push(registered_window);
    pump_messages_until_idle("native close follow-up");
    assert!(
        is_registered(&platform, hwnd),
        "no delayed raw-HWND cleanup may delete a later registry entry that reused the value"
    );
    platform
        .raw_window_handles
        .write()
        .retain(|registered| registered.as_raw() != hwnd);
    assert!(
        !native_window.destroy_native_window(),
        "destroying an already destroyed native window must be idempotent"
    );

    let input_diagnostics = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .terminal
            .into_iter()
            .filter(|diagnostic| {
                diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
            })
            .collect::<Vec<_>>()
    });
    assert!(
        input_diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::Delivered {
                input_result: Some(result),
            } if result.propagate
        )),
        "native input diagnostics must record a propagated delivery"
    );
    assert!(
        input_diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::Delivered {
                input_result: Some(result),
            } if !result.propagate
        )),
        "native input diagnostics must record a consumed delivery"
    );
    assert!(input_diagnostics.iter().all(|diagnostic| matches!(
        diagnostic.disposition,
        NativeBoundaryDisposition::Delivered { .. }
    )));
}

#[test]
fn owned_nonactivating_first_show_preserves_z_order_and_later_activation() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let owner = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("owner window should open");
    let owner_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("owner should register an HWND")
        .as_raw();
    let foreground = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(300.0), px(200.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("foreground sentinel should open");
    let foreground_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("foreground sentinel should register an HWND")
        .as_raw();
    unsafe {
        let _ = SetActiveWindow(foreground_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);
    let native_foreground_before = unsafe { GetForegroundWindow() };
    let z_order_before = application_window_z_order(&platform);
    assert_eq!(
        z_order_before,
        [foreground_hwnd, owner_hwnd],
        "the foreground sentinel must begin directly above the owner in application z-order"
    );

    let transient_owner = app.update_for_test(|cx| {
        cx.transient_window_owner(owner.into())
            .expect("the live owner should produce a transient-owner token")
    });
    let child = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    kind: WindowKind::Floating,
                    window_bounds: Some(WindowBounds::centered(size(px(260.0), px(180.0)), cx)),
                    focus_on_appearing: false,
                    transient_for: Some(transient_owner),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("ordinary owned detached window should open");
    let child_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("ordinary owned child should register an HWND")
        .as_raw();

    assert_eq!(
        unsafe { GetWindow(child_hwnd, GW_OWNER) }
            .expect("the ordinary child should retain a native owner"),
        owner_hwnd
    );
    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);
    assert_eq!(
        unsafe { GetForegroundWindow() },
        native_foreground_before,
        "showing the owned child must not steal global foreground ownership"
    );
    let z_order_after = application_window_z_order(&platform);
    assert_eq!(
        z_order_after
            .iter()
            .copied()
            .filter(|hwnd| *hwnd != child_hwnd)
            .collect::<Vec<_>>(),
        z_order_before,
        "inserting the owned child must not reorder any pre-existing application HWND"
    );
    assert_eq!(
        z_order_after,
        [child_hwnd, foreground_hwnd, owner_hwnd],
        "the visible child must sit above the active sentinel while its owner remains in place"
    );
    let child_ex_style = unsafe { get_window_long(child_hwnd, GWL_EXSTYLE) } as u32;
    assert_eq!(
        child_ex_style & WS_EX_NOACTIVATE.0,
        0,
        "a non-activating first show must not install permanent WS_EX_NOACTIVATE"
    );
    let platform_facts = app.update_for_test(|cx| {
        child
            .update(cx, |_, window, _| window.platform_facts().clone())
            .expect("ordinary owned child should remain live")
    });
    assert!(platform_facts.accepts_activation);
    assert!(platform_facts.focus_on_click);

    let mouse_activate_result = unsafe {
        SendMessageW(
            child_hwnd,
            WM_MOUSEACTIVATE,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
    assert_ne!(mouse_activate_result.0, MA_NOACTIVATE as isize);
    assert_eq!(unsafe { GetActiveWindow() }, child_hwnd);

    unsafe {
        let _ = SetActiveWindow(foreground_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);
    app.update_for_test(|cx| {
        child
            .update(cx, |_, window, _| window.activate_window())
            .expect("ordinary owned child should remain live")
    });
    platform.inner.run_foreground_task();
    pump_messages_until("ordinary owned programmatic activation", || unsafe {
        GetActiveWindow() == child_hwnd
    });
    pump_messages_until_idle("ordinary owned activation follow-up");

    for handle in [
        AnyWindowHandle::from(child),
        AnyWindowHandle::from(foreground),
        AnyWindowHandle::from(owner),
    ] {
        app.update_for_test(|cx| {
            handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("ordinary owned activation test window should close");
        });
        pump_messages_until_idle("ordinary owned activation test teardown");
    }
}

#[test]
fn owned_nonactivating_maximized_first_show_preserves_focus_and_restore_bounds() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let owner = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("owner window should open");
    let owner_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("owner should register an HWND")
        .as_raw();
    let restore_bounds = platform
        .window_from_hwnd(owner_hwnd)
        .expect("owner should remain registered")
        .observed_platform_facts_for_test()
        .expect("owner native facts should be readable")
        .window_bounds
        .get_bounds();
    let reference = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    kind: WindowKind::Floating,
                    window_bounds: Some(WindowBounds::Maximized(restore_bounds)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("ordinary activating maximized reference should open");
    let reference_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("maximized reference should register an HWND")
        .as_raw();
    pump_messages_until("ordinary activating maximized reference", || unsafe {
        IsWindowVisible(reference_hwnd).as_bool() && IsZoomed(reference_hwnd).as_bool()
    });
    pump_messages_until_idle("ordinary activating maximized reference follow-up");
    let reference_outer_bounds = native_window_rect(reference_hwnd);
    let reference_facts = platform
        .window_from_hwnd(reference_hwnd)
        .expect("maximized reference should remain registered")
        .observed_platform_facts_for_test()
        .expect("maximized reference native facts should be readable");
    assert!(reference_facts.is_maximized);
    assert_eq!(
        reference_facts.window_bounds,
        WindowBounds::Maximized(restore_bounds)
    );
    let reference_handle = AnyWindowHandle::from(reference);
    app.update_for_test(|cx| {
        reference_handle
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("maximized reference should close");
    });
    pump_messages_until("maximized reference teardown", || {
        !unsafe { IsWindow(Some(reference_hwnd)).as_bool() }
            && !is_registered(&platform, reference_hwnd)
    });

    let foreground = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(300.0), px(200.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("foreground sentinel should open");
    let foreground_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("foreground sentinel should register an HWND")
        .as_raw();
    unsafe {
        let _ = SetActiveWindow(foreground_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);

    let transient_owner = app.update_for_test(|cx| {
        cx.transient_window_owner(owner.into())
            .expect("the live owner should produce a transient-owner token")
    });
    let child = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    kind: WindowKind::Floating,
                    window_bounds: Some(WindowBounds::Maximized(restore_bounds)),
                    focus_on_appearing: false,
                    transient_for: Some(transient_owner),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("maximized owned child should open");
    let child_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("maximized owned child should register an HWND")
        .as_raw();

    pump_messages_until("non-activating maximized initial presentation", || unsafe {
        IsWindowVisible(child_hwnd).as_bool() && IsZoomed(child_hwnd).as_bool()
    });
    pump_messages_until_idle("non-activating maximized initial presentation follow-up");

    assert_eq!(
        unsafe { GetWindow(child_hwnd, GW_OWNER) }
            .expect("the maximized child should retain a native owner"),
        owner_hwnd
    );
    assert_eq!(
        unsafe { GetActiveWindow() },
        foreground_hwnd,
        "a non-activating maximized first show must preserve the active window"
    );
    assert!(unsafe { IsZoomed(child_hwnd) }.as_bool());
    let child_ex_style = unsafe { get_window_long(child_hwnd, GWL_EXSTYLE) } as u32;
    assert_eq!(
        child_ex_style & WS_EX_NOACTIVATE.0,
        0,
        "a non-activating maximized first show must not install permanent WS_EX_NOACTIVATE"
    );

    let native_window = platform
        .window_from_hwnd(child_hwnd)
        .expect("maximized child should remain registered");
    let native_facts = native_window
        .observed_platform_facts_for_test()
        .expect("maximized child native facts should be readable");
    assert!(native_facts.is_maximized);
    assert_eq!(
        native_window_rect(child_hwnd),
        reference_outer_bounds,
        "non-activating maximization must use the ordinary native maximized outer bounds"
    );
    assert_eq!(
        native_facts.bounds, reference_facts.bounds,
        "non-activating maximization must preserve ordinary native titlebar and client geometry"
    );
    assert_eq!(
        native_facts.window_bounds,
        WindowBounds::Maximized(restore_bounds),
        "non-activating maximization must preserve rcNormalPosition as the restore bounds"
    );
    assert!(native_facts.accepts_activation);
    assert!(native_facts.focus_on_click);

    app.update_for_test(|cx| {
        child
            .update(cx, |_, window, _| window.activate_window())
            .expect("maximized owned child should remain live")
    });
    platform.inner.run_foreground_task();
    pump_messages_until("maximized owned programmatic activation", || unsafe {
        GetActiveWindow() == child_hwnd
    });
    pump_messages_until_idle("maximized owned activation follow-up");
    let activated_ex_style = unsafe { get_window_long(child_hwnd, GWL_EXSTYLE) } as u32;
    assert_eq!(activated_ex_style & WS_EX_NOACTIVATE.0, 0);

    for handle in [
        AnyWindowHandle::from(child),
        AnyWindowHandle::from(foreground),
        AnyWindowHandle::from(owner),
    ] {
        app.update_for_test(|cx| {
            handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("maximized owned activation test window should close");
        });
        pump_messages_until_idle("maximized owned activation test teardown");
    }
}

#[test]
fn asymmetric_activation_policy_preserves_click_and_programmatic_independence() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let owner = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("owner window should open");
    let owner_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("owner should register an HWND")
        .as_raw();

    for policy in [
        WindowActivationPolicy {
            accepts_activation: false,
            focus_on_click: true,
        },
        WindowActivationPolicy {
            accepts_activation: true,
            focus_on_click: false,
        },
    ] {
        unsafe {
            let _ = SetActiveWindow(owner_hwnd);
        }
        assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);
        let transient_owner = app.update_for_test(|cx| {
            cx.transient_window_owner(owner.into())
                .expect("the live owner should produce a transient-owner token")
        });
        let child = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        kind: WindowKind::Floating,
                        window_bounds: Some(WindowBounds::centered(size(px(260.0), px(180.0)), cx)),
                        focus_on_appearing: false,
                        activation_policy: policy,
                        transient_for: Some(transient_owner),
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("asymmetric-policy child should open");
        let child_hwnd = platform
            .raw_window_handles
            .read()
            .last()
            .expect("child should register an HWND")
            .as_raw();

        let facts = app.update_for_test(|cx| {
            child
                .update(cx, |_, window, _| window.platform_facts().clone())
                .expect("asymmetric-policy child should remain live")
        });
        assert_eq!(facts.accepts_activation, policy.accepts_activation);
        assert_eq!(facts.focus_on_click, policy.focus_on_click);
        let child_ex_style = unsafe { get_window_long(child_hwnd, GWL_EXSTYLE) } as u32;
        assert_eq!(
            child_ex_style & WS_EX_NOACTIVATE.0 == 0,
            policy.focus_on_click,
            "the native no-activate style must project click focus only"
        );

        let mouse_activate_result = unsafe {
            SendMessageW(
                child_hwnd,
                WM_MOUSEACTIVATE,
                Some(WPARAM::default()),
                Some(LPARAM::default()),
            )
        };
        if policy.focus_on_click {
            assert_ne!(mouse_activate_result.0, MA_NOACTIVATE as isize);
            assert_eq!(unsafe { GetActiveWindow() }, child_hwnd);
        } else {
            assert_eq!(mouse_activate_result.0, MA_NOACTIVATE as isize);
            assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);
        }

        unsafe {
            let _ = SetActiveWindow(owner_hwnd);
        }
        app.update_for_test(|cx| {
            child
                .update(cx, |_, window, _| window.activate_window())
                .expect("asymmetric-policy child should remain live")
        });
        platform.inner.run_foreground_task();
        if policy.accepts_activation {
            pump_messages_until("asymmetric programmatic activation", || unsafe {
                GetActiveWindow() == child_hwnd
            });
        } else {
            pump_messages_until_idle("rejected asymmetric programmatic activation");
            assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);
        }

        app.update_for_test(|cx| {
            child
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("asymmetric-policy child should close");
        });
        pump_messages_until_idle("asymmetric-policy child teardown");
    }

    app.update_for_test(|cx| {
        owner
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("owner should close");
    });
    pump_messages_until_idle("asymmetric activation test teardown");
}

#[test]
fn owned_permanently_nonactivating_window_preserves_owner_and_rejects_activation() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let owner = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("owner window should open");
    let owner_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("owner should register an HWND")
        .as_raw();
    unsafe {
        let _ = SetActiveWindow(owner_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);
    let transient_owner = app.update_for_test(|cx| {
        cx.transient_window_owner(owner.into())
            .expect("the live owner should produce a transient-owner token")
    });

    let child = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    kind: WindowKind::Floating,
                    window_bounds: Some(WindowBounds::centered(size(px(260.0), px(180.0)), cx)),
                    focus_on_appearing: true,
                    activation_policy: WindowActivationPolicy {
                        accepts_activation: false,
                        focus_on_click: false,
                    },
                    transient_for: Some(transient_owner),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("owned nonactivating window should open");
    let child_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("child should register an HWND")
        .as_raw();

    assert_eq!(
        unsafe { GetWindow(child_hwnd, GW_OWNER) }.expect("the child should retain a native owner"),
        owner_hwnd
    );
    assert_eq!(
        unsafe { GetActiveWindow() },
        owner_hwnd,
        "the owned first show must not activate either window out of order"
    );
    let (creation_facts, platform_facts) = app.update_for_test(|cx| {
        child
            .update(cx, |_, window, _| {
                (
                    window.creation_facts().clone(),
                    window.platform_facts().clone(),
                )
            })
            .expect("the owned child should remain live")
    });
    assert_eq!(creation_facts.transient_for, Some(owner.into()));
    assert!(creation_facts.focus_on_appearing);
    assert!(!platform_facts.accepts_activation);
    assert!(!platform_facts.focus_on_click);

    let mouse_activate_result = unsafe {
        SendMessageW(
            child_hwnd,
            WM_MOUSEACTIVATE,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
    assert_eq!(mouse_activate_result.0, MA_NOACTIVATE as isize);
    assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);

    app.update_for_test(|cx| {
        child
            .update(cx, |_, window, _| window.activate_window())
            .expect("the owned child should remain live")
    });
    platform.inner.run_foreground_task();
    pump_messages_until_idle("rejected owned-child activation follow-up");
    assert_eq!(unsafe { GetActiveWindow() }, owner_hwnd);

    for handle in [AnyWindowHandle::from(child), AnyWindowHandle::from(owner)] {
        app.update_for_test(|cx| {
            handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("owned activation test window should close");
        });
        pump_messages_until_idle("owned activation test teardown");
    }
}

#[test]
fn real_hwnd_initial_presentation_is_post_commit_idle_and_retries_rejection() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let synchronous_results = Rc::new(RefCell::new(Vec::new()));
    platform
        .lifecycle_test_probe
        .install_initial_presentation_hook({
            let synchronous_results = synchronous_results.clone();
            move |hwnd| {
                assert!(
                    unsafe { !IsWindowVisible(hwnd).as_bool() },
                    "the HWND must remain hidden until the committed presentation command runs"
                );
                let propagated = unsafe {
                    SendMessageW(
                        hwnd,
                        WM_MOUSEMOVE,
                        Some(WPARAM::default()),
                        Some(mouse_position_lparam(12, 18)),
                    )
                };
                let consumed = unsafe {
                    SendMessageW(
                        hwnd,
                        WM_MOUSEMOVE,
                        Some(WPARAM::default()),
                        Some(mouse_position_lparam(24, 30)),
                    )
                };
                synchronous_results
                    .borrow_mut()
                    .extend([propagated.0, consumed.0]);
            }
        });
    platform
        .lifecycle_test_probe
        .fail_next_initial_presentation();

    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let interceptor = Rc::new(RefCell::new(None));
    let builder_observed_hidden = Rc::new(Cell::new(false));
    let input_count = Rc::new(Cell::new(0usize));
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                {
                    let platform = platform.clone();
                    let interceptor = interceptor.clone();
                    let builder_observed_hidden = builder_observed_hidden.clone();
                    let input_count = input_count.clone();
                    move |window, cx| {
                        let hwnd = platform
                            .lifecycle_test_probe
                            .last_created_hwnd()
                            .expect("the root builder should observe its newly created HWND");
                        assert!(
                            unsafe { !IsWindowVisible(hwnd).as_bool() },
                            "the root builder must run before native presentation"
                        );
                        builder_observed_hidden.set(true);
                        interceptor
                            .borrow_mut()
                            .replace(window.intercept_window_mouse_events(
                                move |event, window, cx| {
                                    if !matches!(event, WindowMouseEvent::Move(_)) {
                                        return;
                                    }
                                    let delivery = input_count.get().saturating_add(1);
                                    input_count.set(delivery);
                                    if delivery == 2 {
                                        cx.stop_propagation();
                                        window.prevent_default();
                                    }
                                },
                            ));
                        cx.new(|_| Empty)
                    }
                },
            )
        })
        .expect("native test window should open after one rejected presentation attempt");
    let hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the presentation test should retain its HWND");

    assert!(builder_observed_hidden.get());
    assert_eq!(
        synchronous_results.borrow().as_slice(),
        &[1, 0],
        "the post-borrow command must return the exact propagated and consumed Win32 dispositions"
    );
    assert_eq!(input_count.get(), 2);
    assert!(
        unsafe { IsWindowVisible(hwnd).as_bool() },
        "the bounded retry must eventually present the committed HWND"
    );

    let target = NativeBoundaryTarget::Window(window.window_id());
    let diagnostics = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .terminal
    });
    let presentation_dispositions = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == target
                && diagnostic.kind
                    == NativeBoundaryKind::Command(
                        NativePlatformCommandKind::CompleteInitialPresentation,
                    )
        })
        .map(|diagnostic| diagnostic.disposition)
        .collect::<Vec<_>>();
    assert_eq!(
        presentation_dispositions,
        [
            NativeBoundaryDisposition::Rejected,
            NativeBoundaryDisposition::Delivered { input_result: None },
        ],
        "a failed attempt must be terminally rejected before the exact command is retried"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == target
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(
                            NativeCallbackKind::InitialPresentationCompleted,
                        )
                    && diagnostic.disposition
                        == NativeBoundaryDisposition::Delivered { input_result: None }
            })
            .count(),
        1,
        "only the accepted presentation attempt may publish completion"
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.target != target
            || !matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(_)
            )
    }));

    let any_window = AnyWindowHandle::from(window);
    app.update_for_test(|cx| any_window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("presentation test window should close");
    pump_messages_until("presentation test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn failed_forced_initial_presentation_rejects_activation_and_rolls_back_deferred_placement() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let foreground = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(300.0), px(200.0)), cx)),
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("foreground sentinel should open");
    let foreground_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("foreground sentinel should register an HWND")
        .as_raw();
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(340.0), px(240.0)), cx)),
                    focus_on_appearing: false,
                    show: false,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("hidden activation target should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("hidden activation target should register an HWND")
        .as_raw();
    assert!(!unsafe { IsWindowVisible(hwnd).as_bool() });
    unsafe {
        let _ = SetActiveWindow(foreground_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);

    let initial_window_bounds = platform
        .window_from_hwnd(hwnd)
        .expect("hidden activation target should remain registered")
        .observed_platform_facts_for_test()
        .expect("hidden activation target native facts should be readable")
        .window_bounds;
    assert!(matches!(initial_window_bounds, WindowBounds::Windowed(_)));
    let placement_ticket = app.update_for_test(|cx| {
        window
            .update(cx, |_, window, _| {
                match window.request_window_placement(WindowBounds::Maximized(
                    initial_window_bounds.get_bounds(),
                )) {
                    WindowMutationDispatch::Queued(ticket) => ticket,
                    dispatch => panic!("expected deferred placement ticket, got {dispatch:?}"),
                }
            })
            .expect("hidden activation target should remain live")
    });
    assert!(placement_ticket.observation().is_none());
    platform
        .lifecycle_test_probe
        .fail_next_initial_presentation();
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });

    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, _| window.activate_window())
            .expect("hidden activation target should remain live")
    });
    platform.inner.run_foreground_task();
    pump_messages_until("rejected forced initial presentation", || {
        placement_ticket.observation().is_some()
    });
    pump_messages_until_idle("rejected forced initial presentation follow-up");

    assert_eq!(unsafe { GetActiveWindow() }, foreground_hwnd);
    assert!(
        !unsafe { IsWindowVisible(hwnd).as_bool() },
        "a rejected forced presentation must leave the target hidden"
    );
    let placement_observation = placement_ticket
        .observation()
        .expect("the deferred placement ticket must settle on presentation failure");
    assert_eq!(
        placement_observation.outcome,
        WindowMutationOutcome::Rejected
    );
    assert_eq!(
        placement_observation.facts.window_bounds,
        initial_window_bounds
    );
    assert!(!placement_observation.facts.is_maximized);

    let target = NativeBoundaryTarget::Window(window.window_id());
    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    let activation_dispositions = diagnostic_delta
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == target
                && diagnostic.kind
                    == NativeBoundaryKind::Command(NativePlatformCommandKind::Activate)
        })
        .map(|diagnostic| diagnostic.disposition)
        .collect::<Vec<_>>();
    assert_eq!(
        activation_dispositions,
        [NativeBoundaryDisposition::Rejected],
        "activation must reject immediately after forced presentation fails"
    );

    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, _| window.activate_window())
            .expect("hidden activation target should remain live")
    });
    platform.inner.run_foreground_task();
    pump_messages_until("retried activation after presentation failure", || unsafe {
        IsWindowVisible(hwnd).as_bool() && GetActiveWindow() == hwnd
    });
    pump_messages_until_idle("retried activation follow-up");
    let retry_facts = platform
        .window_from_hwnd(hwnd)
        .expect("retried activation target should remain registered")
        .observed_platform_facts_for_test()
        .expect("retried activation target native facts should be readable");
    assert_eq!(retry_facts.window_bounds, initial_window_bounds);
    assert!(!retry_facts.is_maximized);

    for handle in [
        AnyWindowHandle::from(window),
        AnyWindowHandle::from(foreground),
    ] {
        app.update_for_test(|cx| {
            handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("forced-presentation activation test window should close");
        });
        pump_messages_until_idle("forced-presentation activation test teardown");
    }
}

#[test]
fn real_hwnd_reserved_callbacks_deliver_after_commit_and_retire_after_rollback() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let success_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let success_id = Rc::new(Cell::new(None));
    let success_pending_observed = Rc::new(Cell::new(false));
    let successful_window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: false,
                    ..WindowOptions::default()
                },
                {
                    let platform = platform.clone();
                    let success_id = success_id.clone();
                    let success_pending_observed = success_pending_observed.clone();
                    move |window, cx| {
                        let window_id = window.window_handle().window_id();
                        success_id.set(Some(window_id));
                        let hwnd = platform
                            .lifecycle_test_probe
                            .last_created_hwnd()
                            .expect("the committed reservation should have a native HWND");
                        unsafe {
                            SendMessageW(
                                hwnd,
                                WM_MOVE,
                                Some(WPARAM::default()),
                                Some(mouse_position_lparam(40, 50)),
                            );
                        }
                        let diagnostics = cx.native_boundary_diagnostics(success_cursor);
                        assert!(diagnostics.pending.iter().any(|diagnostic| {
                            diagnostic.target == NativeBoundaryTarget::Window(window_id)
                                && diagnostic.kind
                                    == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
                        }));
                        assert!(!diagnostics.terminal.iter().any(|diagnostic| {
                            diagnostic.target == NativeBoundaryTarget::Window(window_id)
                                && diagnostic.kind
                                    == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
                                && matches!(
                                    diagnostic.disposition,
                                    NativeBoundaryDisposition::Delivered { .. }
                                        | NativeBoundaryDisposition::Stale
                                        | NativeBoundaryDisposition::Closed
                                )
                        }));
                        success_pending_observed.set(true);
                        cx.new(|_| Empty)
                    }
                },
            )
        })
        .expect("the committed reservation should open");
    let successful_hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the committed reservation should retain its HWND");
    let success_id = success_id
        .get()
        .expect("the root builder should capture the committed reservation id");
    assert!(success_pending_observed.get());
    let success_diagnostics =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(success_cursor).terminal);
    assert!(success_diagnostics.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(success_id)
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
            && diagnostic.disposition == NativeBoundaryDisposition::Delivered { input_result: None }
    }));

    let rollback_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let rollback_id = Rc::new(Cell::new(None));
    let rollback_pending_observed = Rc::new(Cell::new(false));
    let rollback_result = app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(280.0), px(180.0)), cx)),
                focus_on_appearing: false,
                show: false,
                ..WindowOptions::default()
            },
            {
                let platform = platform.clone();
                let rollback_id = rollback_id.clone();
                let rollback_pending_observed = rollback_pending_observed.clone();
                move |window, cx| {
                    let window_id = window.window_handle().window_id();
                    rollback_id.set(Some(window_id));
                    let hwnd = platform
                        .lifecycle_test_probe
                        .last_created_hwnd()
                        .expect("the rolled-back reservation should have a native HWND");
                    unsafe {
                        SendMessageW(
                            hwnd,
                            WM_MOVE,
                            Some(WPARAM::default()),
                            Some(mouse_position_lparam(60, 70)),
                        );
                    }
                    let diagnostics = cx.native_boundary_diagnostics(rollback_cursor);
                    assert!(diagnostics.pending.iter().any(|diagnostic| {
                        diagnostic.target == NativeBoundaryTarget::Window(window_id)
                            && diagnostic.kind
                                == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
                    }));
                    rollback_pending_observed.set(true);
                    window.remove_window(cx);
                    cx.new(|_| Empty)
                }
            },
        )
    });
    assert!(
        rollback_result.is_err(),
        "a window removed by its root builder must roll back its reservation"
    );
    let rollback_hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the rollback probe should retain the destroyed HWND value");
    let rollback_id = rollback_id
        .get()
        .expect("the root builder should capture the rolled-back reservation id");
    assert!(rollback_pending_observed.get());
    assert!(unsafe { !IsWindow(Some(rollback_hwnd)).as_bool() });
    assert!(!is_registered(&platform, rollback_hwnd));
    let rollback_diagnostics =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(rollback_cursor).terminal);
    assert!(rollback_diagnostics.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(rollback_id)
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
            && matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Stale | NativeBoundaryDisposition::Closed
            )
    }));
    assert!(!rollback_diagnostics.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(rollback_id)
            && diagnostic.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::InitialPresentationCompleted)
    }));

    let successful_window = AnyWindowHandle::from(successful_window);
    app.update_for_test(|cx| {
        successful_window.update(cx, |_, window, cx| window.remove_window(cx))
    })
    .expect("the committed reservation test window should close");
    pump_messages_until("reserved-window test teardown", || {
        !unsafe { IsWindow(Some(successful_hwnd)).as_bool() }
            && !is_registered(&platform, successful_hwnd)
    });
}

#[test]
fn repeated_native_minimize_preserves_frame_callback_through_restore() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let frame_requests = observe_frame_requests(&native_window);
    pump_messages_until_idle("initial native window messages");

    post_message(
        hwnd,
        WM_SIZE,
        WPARAM(SIZE_MINIMIZED as usize),
        LPARAM::default(),
    );
    post_message(
        hwnd,
        WM_SIZE,
        WPARAM(SIZE_MINIMIZED as usize),
        LPARAM::default(),
    );
    pump_messages_until_idle("repeated native minimize");
    assert!(
        native_window.state.callbacks.request_frame.take().is_none(),
        "the frame callback must remain suspended while minimized"
    );

    post_message(
        hwnd,
        WM_SIZE,
        WPARAM(SIZE_RESTORED as usize),
        mouse_position_lparam(320, 220),
    );
    pump_messages_until_idle("native restore");
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let frame_request_baseline = frame_requests.get();
    assert!(
        unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() },
        "restored native test window should accept paint invalidation"
    );
    let paint_trace = pump_messages_until("restored native WM_PAINT", || {
        frame_requests.get() > frame_request_baseline
            && app.update_for_test(|cx| {
                cx.native_boundary_diagnostics(diagnostic_cursor)
                    .terminal
                    .iter()
                    .any(|diagnostic| {
                        diagnostic.kind
                            == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
                            && diagnostic.disposition
                                == NativeBoundaryDisposition::Delivered { input_result: None }
                    })
            })
    });
    assert!(
        paint_trace.contains(hwnd, WM_PAINT),
        "the restored frame request must originate from WM_PAINT"
    );

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native test window should close");
    pump_messages_until("repeated-minimize test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn native_mouse_leave_dispatches_exact_input_before_hover_fact() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let consume_exit = Rc::new(Cell::new(false));
    let exit_count = Rc::new(Cell::new(0usize));
    let exit_positions = Rc::new(RefCell::new(Vec::new()));
    let exit_had_physical_frame = Rc::new(RefCell::new(Vec::new()));
    let _mouse_interceptor = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let consume_exit = consume_exit.clone();
                    let exit_count = exit_count.clone();
                    let exit_positions = exit_positions.clone();
                    let exit_had_physical_frame = exit_had_physical_frame.clone();
                    let native_window = native_window.clone();
                    move |event, window, cx| {
                        if let WindowMouseEvent::Exit(event) = event {
                            exit_count.set(exit_count.get().saturating_add(1));
                            exit_positions.borrow_mut().push(event.position);
                            exit_had_physical_frame.borrow_mut().push(
                                native_window
                                    .native_pointer_physical_frame_for_test()
                                    .is_some(),
                            );
                            if consume_exit.get() {
                                cx.stop_propagation();
                                window.prevent_default();
                            }
                        }
                    }
                })
            })
        })
        .expect("native test window should accept a mouse interceptor");
    pump_messages_until_idle("initial mouse-leave test messages");

    let initial_move = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM::default()),
            Some(mouse_position_lparam(20, 24)),
        )
    };
    assert_eq!(initial_move.0, 1);
    assert!(native_window.state.hovered.get());
    platform.inner.run_foreground_task();
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });

    exit_count.set(0);
    native_window.state.hovered.set(true);
    let propagated_leave = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSELEAVE,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
    assert_eq!(
        propagated_leave.0, 1,
        "a propagated MouseExited input must return the propagated Win32 disposition"
    );
    assert_eq!(exit_count.get(), 1);

    let second_move = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM::default()),
            Some(mouse_position_lparam(28, 32)),
        )
    };
    assert_eq!(second_move.0, 1);
    assert!(native_window.state.hovered.get());
    consume_exit.set(true);
    let consumed_leave = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSELEAVE,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
    assert_eq!(
        consumed_leave.0, 0,
        "a consumed MouseExited input must return the consumed Win32 disposition"
    );
    assert_eq!(exit_count.get(), 2);
    pump_messages_until_idle("consumed mouse-leave follow-up");

    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert!(diagnostic_delta.terminal.iter().all(|diagnostic| {
        diagnostic.target != open_gpui::NativeBoundaryTarget::Window(window.window_id())
            || !matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(_)
            )
    }));
    let input_and_hover = diagnostic_delta
        .terminal
        .iter()
        .filter_map(|diagnostic| match diagnostic.kind {
            NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput) => Some("input"),
            NativeBoundaryKind::Callback(NativeCallbackKind::HoverChanged) => Some("hover"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        input_and_hover,
        ["input", "hover", "input", "hover", "input", "hover"],
        "each native pointer transition must publish exact input before its hover fact"
    );
    let scale_factor = native_window.state.scale_factor.get();
    assert_eq!(
        exit_positions.borrow().as_slice(),
        [
            point(px(20.0 / scale_factor), px(24.0 / scale_factor)),
            point(px(28.0 / scale_factor), px(32.0 / scale_factor)),
        ],
        "mouse leave must reuse the last callback-owned client position"
    );
    assert_eq!(
        exit_had_physical_frame.borrow().as_slice(),
        [false, false],
        "mouse leave must remain hover-only and expose no captured routing frame"
    );

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native test window should close");
    pump_messages_until("mouse-leave test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn nested_native_close_is_prevented_while_should_close_slot_is_checked_out() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let handler_count = Rc::new(Cell::new(0usize));
    let nested_result = Rc::new(Cell::new(None));
    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, app| {
                window.on_window_should_close(app, {
                    let handler_count = handler_count.clone();
                    let nested_result = nested_result.clone();
                    move |_, _| {
                        handler_count.set(handler_count.get().saturating_add(1));
                        let result = unsafe {
                            SendMessageW(
                                hwnd,
                                WM_CLOSE,
                                Some(WPARAM::default()),
                                Some(LPARAM::default()),
                            )
                        };
                        nested_result.set(Some(result.0));
                        false
                    }
                });
            })
            .expect("native test window should remain live")
    });

    post_message(hwnd, WM_CLOSE, WPARAM::default(), LPARAM::default());
    let close_trace = pump_messages_until("nested native WM_CLOSE", || handler_count.get() == 1);
    assert_eq!(close_trace.last_result(hwnd, WM_CLOSE), Some(0));
    assert_eq!(nested_result.get(), Some(0));
    assert!(
        unsafe { IsWindow(Some(hwnd)).as_bool() },
        "nested WM_CLOSE must not bypass a checked-out should-close callback"
    );
    assert!(is_registered(&platform, hwnd));

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native test window should close");
    pump_messages_until("nested-close test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn failed_native_destroy_keeps_window_registered_and_callbacks_live() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let handler_count = Rc::new(Cell::new(0usize));
    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, app| {
                window.on_window_should_close(app, {
                    let handler_count = handler_count.clone();
                    move |_, _| {
                        handler_count.set(handler_count.get().saturating_add(1));
                        false
                    }
                });
            })
            .expect("native test window should remain live")
    });

    platform.lifecycle_test_probe.fail_next_destroy();
    assert!(
        !native_window.destroy_native_window(),
        "injected DestroyWindow failure must be observable"
    );
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(is_registered(&platform, hwnd));
    post_message(hwnd, WM_CLOSE, WPARAM::default(), LPARAM::default());
    let close_trace = pump_messages_until("close after failed native destroy", || {
        handler_count.get() == 1
    });
    assert_eq!(close_trace.last_result(hwnd, WM_CLOSE), Some(0));
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native test window should close after retry");
    pump_messages_until("failed-destroy test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn failed_destroy_during_platform_window_retirement_retries_without_losing_owner() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();

    platform.lifecycle_test_probe.fail_next_destroy();
    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("GPUI window removal should finish despite the injected native teardown failure");

    pump_messages_until("failed native retirement retry", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
    assert!(
        unsafe { !IsWindow(Some(hwnd)).as_bool() },
        "the App-owned retirement retry must destroy the retained child HWND"
    );
    assert!(!is_registered(&platform, hwnd));
}

#[test]
fn framework_pointer_capture_release_is_native_terminal_without_duplicate_cancel() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _mouse_interceptor = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let observed = observed.clone();
                    move |event, _, _| match event {
                        WindowMouseEvent::Down(_) => observed.borrow_mut().push(None),
                        WindowMouseEvent::Cancel(event) => {
                            observed.borrow_mut().push(Some(event.reason));
                        }
                        _ => {}
                    }
                })
            })
        })
        .expect("framework capture-release test should install a mouse interceptor");
    pump_messages_until_idle("initial framework capture-release test messages");

    unsafe {
        let _ = SetActiveWindow(hwnd);
    }
    pump_messages_until("framework capture-release native activation", || unsafe {
        GetActiveWindow() == hwnd
    });
    post_message(
        hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(24, 28),
    );
    pump_messages_until("framework capture-release acquisition", || unsafe {
        GetCapture() == hwnd
    });

    native_window.dispatch_input(PlatformInput::PointerCanceled(PointerCancelEvent {
        reason: PointerCancelReason::CaptureRevoked,
    }));
    pump_messages_until("framework capture-release follow-up", || unsafe {
        GetCapture() != hwnd
    });

    assert_ne!(unsafe { GetCapture() }, hwnd);
    assert_eq!(
        native_window.state.pointer_capture.get(),
        Default::default()
    );
    assert_eq!(native_window.state.pressed_caption_button.get(), None);
    assert_eq!(
        observed.borrow().as_slice(),
        &[None, Some(PointerCancelReason::CaptureRevoked)],
        "the synchronous WM_CAPTURECHANGED notification must not emit a second cancellation"
    );
    assert_eq!(
        native_window
            .state
            .pointer_capture_release_history
            .borrow()
            .len(),
        1,
        "the post-borrow release channel must invoke the native release callback exactly once"
    );

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("framework capture-release native test window should close");
    pump_messages_until("framework capture-release native test teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn capture_acquisition_loss_reentrant_to_set_capture_cancels_after_mouse_down() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let source = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("source native test window should open");
    let source_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("source native test window should register an HWND")
        .as_raw();
    let target = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(300.0), px(200.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("target native test window should open");
    let target_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("target native test window should register an HWND")
        .as_raw();
    let source_native = platform
        .window_from_hwnd(source_hwnd)
        .expect("source native test window should remain registered");
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _mouse_interceptor = app
        .update_for_test(|cx| {
            source.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let observed = observed.clone();
                    move |event, _, _| match event {
                        WindowMouseEvent::Down(_) => observed.borrow_mut().push(None),
                        WindowMouseEvent::Cancel(event) => {
                            observed.borrow_mut().push(Some(event.reason));
                        }
                        _ => {}
                    }
                })
            })
        })
        .expect("reentrant capture-acquisition test should install a mouse interceptor");
    source_native
        .state
        .replace_next_pointer_capture_acquisition_with
        .set(Some(target_hwnd));
    pump_messages_until_idle("initial reentrant capture-acquisition test messages");
    unsafe {
        let _ = SetActiveWindow(source_hwnd);
    }
    pump_messages_until(
        "reentrant capture-acquisition source activation",
        || unsafe { GetActiveWindow() == source_hwnd },
    );

    let result = unsafe {
        SendMessageW(
            source_hwnd,
            WM_LBUTTONDOWN,
            Some(WPARAM(MK_LBUTTON.0 as usize)),
            Some(mouse_position_lparam(24, 28)),
        )
    };
    assert_eq!(result.0, 1);
    pump_messages_until_idle("reentrant capture-acquisition follow-up");

    assert_eq!(unsafe { GetCapture() }, target_hwnd);
    assert_eq!(
        source_native.state.pointer_capture.get(),
        Default::default(),
        "failed acquisition must not leave the source backend session active"
    );
    assert_eq!(source_native.state.input_dispatch.get(), Default::default());
    assert_eq!(
        observed.borrow().as_slice(),
        &[None, Some(PointerCancelReason::PlatformCaptureLost)],
        "capture loss must become terminal only after the matching MouseDown is delivered"
    );

    if unsafe { GetCapture() } == target_hwnd {
        unsafe { ReleaseCapture() }.expect("test cleanup should release replacement capture");
    }
    post_message(
        source_hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until("post-recovery pointer capture acquisition", || unsafe {
        GetCapture() == source_hwnd
    });
    post_message(
        source_hwnd,
        WM_LBUTTONUP,
        WPARAM::default(),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until("post-recovery pointer capture release", || unsafe {
        GetCapture() != source_hwnd
    });
    assert_eq!(
        source_native.state.pointer_capture.get(),
        Default::default()
    );
    assert_eq!(
        observed.borrow().as_slice(),
        &[None, Some(PointerCancelReason::PlatformCaptureLost), None,],
        "a later native pointer session must start and finish normally"
    );

    app.update_for_test(|cx| source.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("source native test window should close");
    app.update_for_test(|cx| target.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("target native test window should close");
    pump_messages_until("reentrant capture-acquisition native test teardown", || {
        !unsafe { IsWindow(Some(source_hwnd)).as_bool() }
            && !unsafe { IsWindow(Some(target_hwnd)).as_bool() }
            && !is_registered(&platform, source_hwnd)
            && !is_registered(&platform, target_hwnd)
    });
}

#[test]
fn deactivation_releases_child_capture_and_cancels_pointer_once() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let source = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("source native test window should open");
    let source_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("source native test window should register an HWND")
        .as_raw();
    let target = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(300.0), px(200.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("target native test window should open");
    let target_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("target native test window should register an HWND")
        .as_raw();
    let source_native = platform
        .window_from_hwnd(source_hwnd)
        .expect("source native test window should remain registered");
    let active_callback_calls = Rc::new(Cell::new(0usize));
    let active_callback_panicked = Rc::new(Cell::new(false));
    let mut active_callback = source_native
        .state
        .callbacks
        .active_status_change
        .take()
        .expect("source native test window should install an active callback");
    source_native
        .state
        .callbacks
        .active_status_change
        .set(Some(Box::new({
            let active_callback_calls = active_callback_calls.clone();
            let active_callback_panicked = active_callback_panicked.clone();
            move |active| {
                active_callback(active);
                active_callback_calls.set(active_callback_calls.get().saturating_add(1));
                if !active && !active_callback_panicked.replace(true) {
                    panic!("injected WA_INACTIVE active callback panic");
                }
            }
        })));
    let cancellations = Rc::new(RefCell::new(Vec::new()));
    let _mouse_interceptor = app
        .update_for_test(|cx| {
            source.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let cancellations = cancellations.clone();
                    move |event, _, _| {
                        if let WindowMouseEvent::Cancel(event) = event {
                            cancellations.borrow_mut().push(event.reason);
                        }
                    }
                })
            })
        })
        .expect("source native test window should accept a mouse interceptor");
    pump_messages_until_idle("initial deactivation-capture test messages");

    unsafe {
        let _ = SetActiveWindow(source_hwnd);
    }
    pump_messages_until("source native activation", || unsafe {
        GetActiveWindow() == source_hwnd
    });
    pump_messages_until_idle("source activation follow-up");
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    post_message(
        source_hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(24, 28),
    );
    pump_messages_until("source native capture acquisition", || unsafe {
        GetCapture() == source_hwnd
    });

    unsafe {
        let _ = SetActiveWindow(target_hwnd);
    }
    pump_messages_until("source native capture release", || unsafe {
        GetActiveWindow() == target_hwnd && GetCapture() != source_hwnd
    });
    pump_messages_until_idle("deactivation capture follow-up");

    assert_eq!(
        &*cancellations.borrow(),
        &[PointerCancelReason::WindowDeactivated],
        "deactivation must terminate the captured pointer session exactly once"
    );
    assert_eq!(
        source_native.state.pointer_capture.get(),
        Default::default()
    );
    assert_eq!(active_callback_calls.get(), 2);
    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert!(diagnostic_delta.terminal.iter().all(|diagnostic| {
        diagnostic.target != NativeBoundaryTarget::Window(source_native.handle.window_id())
            || !matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(_)
            )
    }));

    unsafe {
        let _ = SetActiveWindow(source_hwnd);
    }
    pump_messages_until(
        "source reactivation after panicking deactivation callback",
        || unsafe { GetActiveWindow() == source_hwnd },
    );
    post_message(
        source_hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until(
        "second native pointer session capture acquisition",
        || unsafe { GetCapture() == source_hwnd },
    );
    post_message(
        source_hwnd,
        WM_LBUTTONUP,
        WPARAM::default(),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until("second native pointer session capture release", || unsafe {
        GetCapture() != source_hwnd
    });
    assert_eq!(
        active_callback_calls.get(),
        3,
        "the active callback must be restored after its panic so G2 activation is delivered"
    );
    assert_eq!(
        source_native.state.pointer_capture.get(),
        Default::default()
    );

    app.update_for_test(|cx| source.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("source native test window should close");
    app.update_for_test(|cx| target.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("target native test window should close");
    pump_messages_until("deactivation-capture test teardown", || {
        !unsafe { IsWindow(Some(source_hwnd)).as_bool() }
            && !unsafe { IsWindow(Some(target_hwnd)).as_bool() }
            && !is_registered(&platform, source_hwnd)
            && !is_registered(&platform, target_hwnd)
    });
    if unsafe { GetCapture() } == source_hwnd {
        unsafe { ReleaseCapture() }.expect("test cleanup should release source capture");
    }
}

#[test]
fn panicking_pointer_cancel_reservation_releases_native_capture_before_abi_recovery() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let callback_count = Rc::new(Cell::new(0usize));
    let g2_phase = Rc::new(Cell::new(false));
    let g2_callback_count = Rc::new(Cell::new(0usize));
    native_window.state.callbacks.set_test_input(Box::new({
        let callback_count = callback_count.clone();
        let g2_phase = g2_phase.clone();
        let g2_callback_count = g2_callback_count.clone();
        move |_| {
            let next = callback_count.get().saturating_add(1);
            callback_count.set(next);
            if next == 2 {
                panic!("injected native input callback panic");
            }
            if g2_phase.get() {
                g2_callback_count.set(g2_callback_count.get().saturating_add(1));
            }
            Default::default()
        }
    }));
    pump_messages_until_idle("initial reservation-panic test messages");

    unsafe {
        let _ = SetActiveWindow(hwnd);
    }
    pump_messages_until("reservation-panic source activation", || unsafe {
        GetActiveWindow() == hwnd
    });
    post_message(
        hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(24, 28),
    );
    pump_messages_until("reservation-panic native capture acquisition", || unsafe {
        GetCapture() == hwnd
    });

    native_window
        .state
        .panic_next_pointer_cancel_reservation
        .set(true);
    let result = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM(MK_LBUTTON.0 as usize)),
            Some(mouse_position_lparam(26, 30)),
        )
    };
    assert_eq!(
        result.0, 0,
        "the ABI boundary must contain the recovery panic"
    );
    assert_ne!(
        unsafe { GetCapture() },
        hwnd,
        "ReleaseCapture must precede a fallible pointer-cancel reservation"
    );
    assert_eq!(
        native_window.state.pointer_capture.get(),
        Default::default(),
        "panic recovery must retire the local capture state before returning to Win32"
    );

    g2_phase.set(true);
    post_message(
        hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until("reservation-panic G2 capture acquisition", || unsafe {
        GetCapture() == hwnd
    });
    post_message(
        hwnd,
        WM_LBUTTONUP,
        WPARAM::default(),
        mouse_position_lparam(30, 34),
    );
    pump_messages_until("reservation-panic G2 capture release", || unsafe {
        GetCapture() != hwnd
    });
    assert!(
        g2_callback_count.get() >= 2,
        "G2 input must receive its down and up callbacks after recovery"
    );

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("reservation-panic native test window should close");
    pump_messages_until("reservation-panic native test teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn programmatic_close_while_captured_is_terminal_and_invariant_free() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native test window should remain registered");
    let window_id = native_window.handle.window_id();
    let close_count = Rc::new(Cell::new(0usize));
    let _close_subscription = app.update_for_test(|cx| {
        cx.on_window_closed({
            let close_count = close_count.clone();
            move |_, closed_window| {
                if closed_window == window_id {
                    close_count.set(close_count.get().saturating_add(1));
                }
            }
        })
    });
    pump_messages_until_idle("initial captured-close test messages");

    unsafe {
        let _ = SetActiveWindow(hwnd);
    }
    pump_messages_until("captured-close native activation", || unsafe {
        GetActiveWindow() == hwnd
    });
    pump_messages_until_idle("captured-close activation follow-up");
    post_message(
        hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam(24, 28),
    );
    pump_messages_until("captured-close acquisition", || unsafe {
        GetCapture() == hwnd
    });
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("captured native test window should close without panicking");
    pump_messages_until("captured native window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() }
            && !is_registered(&platform, hwnd)
            && close_count.get() == 1
    });

    assert_ne!(unsafe { GetCapture() }, hwnd);
    assert_eq!(
        native_window.state.pointer_capture.get(),
        Default::default()
    );
    assert_eq!(close_count.get(), 1);
    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert!(diagnostic_delta.terminal.iter().all(|diagnostic| {
        diagnostic.target != NativeBoundaryTarget::Window(native_window.handle.window_id())
            || !matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(_)
            )
    }));
}

#[test]
fn queued_activate_commands_precede_activation_facts_without_synthetic_keyboard_input() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: true,
                    show: false,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native test window should open");
    let hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("native test window should register an HWND")
        .as_raw();
    pump_messages_until_idle("initial activate-command test messages");
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });

    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.activate_window();
            })
            .expect("native test window should remain live")
    });
    let activation_trace = pump_messages_until("queued native activation", || unsafe {
        GetActiveWindow() == hwnd
    });
    let followup_trace = pump_messages_until_idle("queued activation follow-up");

    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    let target = NativeBoundaryTarget::Window(window.window_id());
    let ordered = diagnostic_delta
        .terminal
        .iter()
        .filter(|diagnostic| diagnostic.target == target)
        .filter_map(|diagnostic| match diagnostic.kind {
            NativeBoundaryKind::Command(NativePlatformCommandKind::Activate) => {
                Some(("command", diagnostic.sequence))
            }
            NativeBoundaryKind::Callback(NativeCallbackKind::ActiveChanged) => {
                Some(("active", diagnostic.sequence))
            }
            NativeBoundaryKind::Callback(NativeCallbackKind::ModifiersChanged) => {
                Some(("modifiers", diagnostic.sequence))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
        ["command", "command", "active", "modifiers"]
    );
    assert!(ordered.windows(2).all(|pair| pair[0].1 < pair[1].1));
    assert!(diagnostic_delta.terminal.iter().any(|diagnostic| {
        diagnostic.target == target
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::ActiveChanged)
            && diagnostic.disposition == NativeBoundaryDisposition::Delivered { input_result: None }
    }));
    for message in [WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP] {
        assert!(
            !activation_trace.contains(hwnd, message) && !followup_trace.contains(hwnd, message),
            "framework activation must not synthesize keyboard input message {message:#x}"
        );
    }

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native test window should close");
    pump_messages_until("activate-command test window teardown", || {
        !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
    });
}

#[test]
fn failed_dialog_construction_rolls_back_hwnd_drag_drop_and_modal_parent() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let parent = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: true,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("modal parent window should open");
    let parent_hwnd = platform
        .raw_window_handles
        .read()
        .last()
        .expect("modal parent should register an HWND")
        .as_raw();
    let transient_owner = app.update_for_test(|cx| {
        cx.transient_window_owner(parent.into())
            .expect("the live parent should produce a transient-owner token")
    });
    unsafe {
        let _ = SetActiveWindow(parent_hwnd);
    }
    assert_eq!(unsafe { GetActiveWindow() }, parent_hwnd);
    assert!(unsafe { IsWindowEnabled(parent_hwnd).as_bool() });

    platform
        .lifecycle_test_probe
        .fail_next_after_drag_drop_registration();
    let failure = app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(240.0), px(160.0)), cx)),
                focus_on_appearing: true,
                show: true,
                kind: WindowKind::Dialog,
                transient_for: Some(transient_owner),
                ..WindowOptions::default()
            },
            |_, cx| cx.new(|_| Empty),
        )
    });
    assert!(
        failure.is_err(),
        "the injected post-drag-drop construction failure must propagate"
    );

    let failed_hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the construction probe should record the failed HWND");
    assert_ne!(failed_hwnd, parent_hwnd);
    assert!(
        unsafe { !IsWindow(Some(failed_hwnd)).as_bool() },
        "construction rollback must synchronously destroy the failed HWND"
    );
    assert!(
        unsafe { IsWindowEnabled(parent_hwnd).as_bool() },
        "construction rollback must restore the modal parent"
    );
    assert_eq!(
        platform.raw_window_handles.read().len(),
        1,
        "a failed child must never enter the committed HWND registry"
    );

    let parent = AnyWindowHandle::from(parent);
    app.update_for_test(|cx| {
        parent
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("modal parent should close");
    });
    pump_messages_until_idle("failed construction cleanup");
}

#[test]
fn app_and_platform_drop_destroy_child_and_message_windows_synchronously() {
    discard_stale_quit_messages();

    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let platform_hwnd = platform.handle;
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    for _ in 0..2 {
        app.update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(240.0), px(160.0)), cx)),
                    focus_on_appearing: false,
                    show: false,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native teardown test window should open");
    }
    let child_hwnds = platform
        .raw_window_handles
        .read()
        .iter()
        .map(|handle| handle.as_raw())
        .collect::<Vec<_>>();
    assert_eq!(child_hwnds.len(), 2);
    assert!(unsafe { IsWindow(Some(platform_hwnd)).as_bool() });

    drop(app);
    assert!(
        child_hwnds
            .iter()
            .all(|hwnd| unsafe { !IsWindow(Some(*hwnd)).as_bool() }),
        "dropping the app must synchronously destroy every child HWND"
    );
    assert!(
        platform.raw_window_handles.read().is_empty(),
        "synchronous child destruction must empty the HWND registry"
    );
    assert!(
        unsafe { IsWindow(Some(platform_hwnd)).as_bool() },
        "the platform message HWND must outlive all child HWNDs"
    );

    drop(platform);
    assert!(
        unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
        "dropping the platform must synchronously destroy its message HWND"
    );
}
