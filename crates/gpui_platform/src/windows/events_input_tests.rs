use super::{
    WM_GPUI_DISPLAY_TOPOLOGY_PUBLISHED, WM_GPUI_FORCE_UPDATE_WINDOW, WindowsImeCancellation,
};
use crate::windows::{
    WindowsPlatform, WindowsWindowInner, native_test_pointer::NativeTestSystemPointerGuard,
    native_test_window::NativeTestOpaqueWindow,
};
use open_gpui::{
    AppContext as _, Application, Bounds, DevicePixels, Empty, GraphicalEnvironment, MouseButton,
    Platform as _, PlatformNativePointerCaptureOwner, QuitMode, RequestFrameOptions, WindowBounds,
    WindowHandle, WindowMouseEvent, WindowOptions, WindowingRequest, point, px, size,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    System::SystemServices::MK_LBUTTON,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::{
            Ime::{GCS_COMPSTR, GCS_RESULTSTR},
            KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture, VK_SHIFT},
        },
        WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowPos,
            WM_ACTIVATE, WM_CHAR, WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
            WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MOUSEMOVE, WM_SETFOCUS,
        },
    },
};

fn native_input_window() -> (
    Application,
    WindowHandle<Empty>,
    Rc<WindowsWindowInner>,
    Rc<WindowsPlatform>,
) {
    let platform = Rc::new(
        WindowsPlatform::new_returning_for_test(false)
            .expect("native input test platform should initialize"),
    );
    futures::executor::block_on(
        platform.request_windowing(WindowingRequest::Windowed(GraphicalEnvironment::detect())),
    )
    .expect("native input test requires an interactive Windows desktop");
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("native input test window should open");
    let hwnd = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                let handle = HasWindowHandle::window_handle(window)
                    .expect("native input test window should have a handle");
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    panic!("native input test window should have a Win32 handle");
                };
                HWND(handle.hwnd.get() as *mut std::ffi::c_void)
            })
        })
        .expect("native input test window should remain registered");
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("native input test HWND should have an exact registered owner");
    (app, window, native_window, platform)
}

#[test]
fn ime_cancel_retires_callbacks_until_the_next_composition_start() {
    let cancellation = WindowsImeCancellation::default();
    assert!(!cancellation.cancel(|| {
        assert!(cancellation.consumes(WM_IME_COMPOSITION));
        false
    }));
    assert!(!cancellation.consumes(WM_IME_COMPOSITION));

    assert!(cancellation.cancel(|| {
        assert!(cancellation.consumes(WM_IME_STARTCOMPOSITION));
        assert!(cancellation.consumes(WM_IME_COMPOSITION));
        assert!(!cancellation.consumes(WM_IME_ENDCOMPOSITION));
        true
    }));
    assert!(cancellation.consumes(WM_IME_COMPOSITION));
    assert!(cancellation.consumes(WM_IME_CHAR));
    assert!(!cancellation.consumes(WM_IME_STARTCOMPOSITION));
    assert!(!cancellation.consumes(WM_IME_COMPOSITION));
    assert!(!cancellation.consumes(WM_IME_CHAR));

    for boundary in [
        WM_KEYDOWN,
        WM_CHAR,
        WM_KILLFOCUS,
        WM_SETFOCUS,
        WM_ACTIVATE,
        WM_INPUTLANGCHANGE,
    ] {
        assert!(cancellation.cancel(|| true));
        assert!(cancellation.consumes(WM_IME_COMPOSITION));
        assert!(cancellation.consumes(WM_IME_CHAR));
        assert!(!cancellation.consumes(boundary));
        assert!(!cancellation.consumes(WM_IME_COMPOSITION));
        assert!(!cancellation.consumes(WM_IME_CHAR));
    }

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cancellation.cancel(|| panic!("simulated ImmNotifyIME unwind"))
    }));
    assert!(panicked.is_err());
    assert!(!cancellation.consumes(WM_IME_COMPOSITION));
    assert!(!cancellation.consumes(WM_IME_STARTCOMPOSITION));
    assert!(cancellation.cancel(|| true));
}

#[test]
fn canceled_ime_messages_do_not_become_native_text_input() {
    let (mut app, window, native_window, _) = native_input_window();
    let send = |message, wparam, lparam| unsafe {
        SendMessageW(native_window.hwnd, message, Some(wparam), Some(lparam))
    };
    let send_composition = |flags| send(WM_IME_COMPOSITION, WPARAM(0), LPARAM(flags));
    let send_ime_char = || send(WM_IME_CHAR, WPARAM('x' as usize), LPARAM(0));
    let high_surrogate = Some(0xD83D);
    native_window.state.pending_surrogate.set(high_surrogate);

    assert!(native_window.state.ime_cancellation.cancel(|| {
        assert_eq!(send_composition(GCS_COMPSTR.0 as isize).0, 0);
        send_ime_char();
        true
    }));
    send(WM_IME_ENDCOMPOSITION, WPARAM(0), LPARAM(0));
    assert_eq!(send_composition(GCS_RESULTSTR.0 as isize).0, 0);
    assert_eq!(send_composition(0).0, 0);
    send_ime_char();
    assert_eq!(native_window.state.pending_surrogate.get(), high_surrogate);

    send(WM_CHAR, WPARAM('x' as usize), LPARAM(0));
    assert_eq!(native_window.state.pending_surrogate.get(), None);
    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .unwrap();
}

#[test]
fn stationary_mouse_moves_preserve_reentry_capture_and_exact_physical_updates() {
    let (mut app, window, native_window, _) = native_input_window();
    let moves = Rc::new(RefCell::new(Vec::new()));
    let exits = Rc::new(RefCell::new(Vec::new()));
    let _subscription = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let moves = moves.clone();
                    let exits = exits.clone();
                    let native_window = native_window.clone();
                    move |event, _, _| match event {
                        WindowMouseEvent::Move(event) => moves.borrow_mut().push((
                            event.position,
                            event.pressed_button,
                            native_window.native_pointer_physical_frame_for_test(),
                        )),
                        WindowMouseEvent::Exit(event) => exits.borrow_mut().push((
                            event.position,
                            native_window.native_pointer_physical_frame_for_test(),
                        )),
                        _ => {}
                    }
                })
            })
        })
        .expect("native input test should install an ordinary mouse interceptor");
    let position = LPARAM((30 << 16) | 24);
    let send = |message, wparam| unsafe {
        SendMessageW(native_window.hwnd, message, Some(wparam), Some(position))
    };

    native_window.state.hovered.set(false);
    native_window.state.last_client_pointer_position.set(None);
    assert_eq!(send(WM_MOUSEMOVE, WPARAM::default()).0, 1);
    assert_eq!(moves.borrow().len(), 1);
    let first_frame = moves.borrow()[0]
        .2
        .expect("real native mouse input should carry its exact physical frame");
    assert_eq!(send(WM_MOUSEMOVE, WPARAM::default()).0, 1);
    assert_eq!(moves.borrow().len(), 1);
    assert!(native_window.state.hovered.get());
    assert_eq!(native_window.native_pointer_physical_frame_for_test(), None);

    send(WM_MOUSEMOVE, WPARAM(MK_LBUTTON.0 as usize));
    send(WM_MOUSEMOVE, WPARAM(MK_LBUTTON.0 as usize));
    assert_eq!(moves.borrow().len(), 3);
    assert_eq!(moves.borrow()[1].1, Some(MouseButton::Left));
    assert_eq!(moves.borrow()[2].1, Some(MouseButton::Left));

    send(WM_LBUTTONDOWN, WPARAM(MK_LBUTTON.0 as usize));
    assert!(
        native_window
            .state
            .pointer_capture
            .get()
            .has_active_session()
    );
    send(WM_MOUSEMOVE, WPARAM::default());
    send(WM_MOUSEMOVE, WPARAM::default());
    assert_eq!(moves.borrow().len(), 5);
    send(WM_LBUTTONUP, WPARAM::default());
    assert!(
        !native_window
            .state
            .pointer_capture
            .get()
            .has_active_session()
    );

    assert_eq!(send(WM_MOUSELEAVE, WPARAM::default()).0, 1);
    assert_eq!(exits.borrow().as_slice(), &[(moves.borrow()[0].0, None)]);
    assert!(!native_window.state.hovered.get());
    assert_eq!(send(WM_MOUSEMOVE, WPARAM::default()).0, 1);
    assert_eq!(moves.borrow().len(), 6);
    assert!(native_window.state.hovered.get());
    send(WM_MOUSEMOVE, WPARAM::default());
    assert_eq!(moves.borrow().len(), 6);

    let mut rect = RECT::default();
    unsafe { GetWindowRect(native_window.hwnd, &mut rect) }
        .expect("native input test should sample the window rectangle");
    unsafe {
        SetWindowPos(
            native_window.hwnd,
            None,
            rect.left + 1,
            rect.top,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
    }
    .expect("native input test should move its source geometry by one device pixel");
    let before_geometry_move = moves.borrow().len();
    send(WM_MOUSEMOVE, WPARAM::default());
    assert_eq!(moves.borrow().len(), before_geometry_move + 1);
    let moved_frame = moves.borrow().last().unwrap().2.unwrap();
    assert_ne!(moved_frame.source_geometry(), first_frame.source_geometry());
    assert_eq!(
        moved_frame.global_position().x.0,
        first_frame.global_position().x.0 + 1,
    );
    assert_eq!(moves.borrow().last().unwrap().0, moves.borrow()[0].0);
    let before_duplicate = moves.borrow().len();
    send(WM_MOUSEMOVE, WPARAM::default());
    assert_eq!(moves.borrow().len(), before_duplicate);
    assert_eq!(native_window.native_pointer_physical_frame_for_test(), None);

    unsafe {
        SendMessageW(
            native_window.hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM::default()),
            Some(LPARAM((30 << 16) | 25)),
        )
    };
    assert_eq!(moves.borrow().len(), before_duplicate + 1);
    assert_ne!(moves.borrow().last().unwrap().0, moves.borrow()[0].0);

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native input test window should retire");
    assert!(native_window.is_native_window_terminal());
}

#[test]
fn native_pointer_frames_retain_callback_capture_and_validate_registered_owners() {
    let (mut app, window, native_window, platform) = native_input_window();
    let other_window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    ..WindowOptions::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
        })
        .expect("capture observation test should open another registered window");
    let other_native_window = app
        .update_for_test(|cx| {
            other_window.update(cx, |_, window, _| {
                let handle = HasWindowHandle::window_handle(window)
                    .expect("capture observation test window should have a handle");
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    panic!("capture observation test window should have a Win32 handle");
                };
                platform
                    .window_from_hwnd(HWND(handle.hwnd.get() as *mut std::ffi::c_void))
                    .expect("capture observation test window should have a registered owner")
            })
        })
        .expect("capture observation test window should remain registered");
    let unknown_window = NativeTestOpaqueWindow::create_hidden(Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(32), DevicePixels(32)),
    ))
    .expect("capture observation test should create an unregistered native window");
    let pointer_guard = NativeTestSystemPointerGuard::capture()
        .expect("capture observation test should retain the original native pointer state");
    let frames = Rc::new(RefCell::new(Vec::new()));
    let replacement_capture = Rc::new(Cell::new(Some(other_native_window.hwnd)));
    let subscription = app
        .update_for_test(|cx| {
            window.update(cx, |_, window, _| {
                window.intercept_window_mouse_events({
                    let frames = frames.clone();
                    let replacement_capture = replacement_capture.clone();
                    let native_window = native_window.clone();
                    move |event, _, _| {
                        if let WindowMouseEvent::Move(_) = event {
                            let frame = native_window.native_pointer_physical_frame_for_test();
                            if let Some(replacement) = replacement_capture.take() {
                                unsafe { SetCapture(replacement) };
                            }
                            frames.borrow_mut().push((
                                frame,
                                native_window.native_pointer_physical_frame_for_test(),
                            ));
                        }
                    }
                })
            })
        })
        .expect("capture observation test should install a native mouse interceptor");
    let send_move = |x: i32| unsafe {
        SendMessageW(
            native_window.hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM(MK_LBUTTON.0 as usize)),
            Some(LPARAM(((30 << 16) | x) as isize)),
        )
    };

    unsafe { SetCapture(native_window.hwnd) };
    assert_eq!(unsafe { GetCapture() }, native_window.hwnd);
    send_move(24);
    assert_eq!(frames.borrow().len(), 1);
    let source_frame = frames.borrow()[0]
        .0
        .expect("the source callback should retain its physical frame");
    assert_eq!(
        source_frame.capture_owner(),
        PlatformNativePointerCaptureOwner::Window(window.window_id()),
    );
    assert_eq!(unsafe { GetCapture() }, other_native_window.hwnd);
    assert_eq!(frames.borrow()[0].1, None);
    assert_eq!(native_window.native_pointer_physical_frame_for_test(), None);

    send_move(25);
    let other_frame = frames.borrow()[1]
        .0
        .expect("the next callback should observe the new capture owner");
    assert_eq!(
        other_frame.capture_owner(),
        PlatformNativePointerCaptureOwner::Window(other_window.window_id()),
    );
    assert_eq!(frames.borrow()[1].1, Some(other_frame));
    assert_eq!(native_window.native_pointer_physical_frame_for_test(), None);

    unsafe { SetCapture(unknown_window.hwnd()) };
    assert_eq!(unsafe { GetCapture() }, unknown_window.hwnd());
    send_move(27);
    assert_eq!(
        frames.borrow()[2].0.unwrap().capture_owner(),
        PlatformNativePointerCaptureOwner::UnregisteredWindow,
    );

    unsafe { ReleaseCapture() }.expect("capture observation test should release native capture");
    assert_eq!(unsafe { GetCapture() }, HWND::default());
    send_move(28);
    assert_eq!(
        frames.borrow()[3].0.unwrap().capture_owner(),
        PlatformNativePointerCaptureOwner::Unavailable,
        "thread-local absence of capture cannot prove desktop-wide absence",
    );
    assert_eq!(frames.borrow().len(), 4);
    assert_eq!(frames.borrow()[0].0, Some(source_frame));
    assert_eq!(frames.borrow()[1].0, Some(other_frame));
    assert_eq!(native_window.native_pointer_physical_frame_for_test(), None);

    drop(subscription);
    drop(pointer_guard);
    app.update_for_test(|cx| {
        other_window.update(cx, |_, window, cx| window.remove_window(cx))?;
        window.update(cx, |_, window, cx| window.remove_window(cx))
    })
    .expect("capture observation test windows should retire");
    assert!(native_window.is_native_window_terminal());
    assert!(other_native_window.is_native_window_terminal());
}

#[test]
fn private_input_messages_require_validation_and_the_current_native_generation() {
    let (mut app, window, native_window, _) = native_input_window();
    let frame_requests = Rc::new(Cell::new(0usize));
    let request_frame: Rc<dyn Fn(RequestFrameOptions)> = Rc::new({
        let frame_requests = frame_requests.clone();
        move |_| frame_requests.set(frame_requests.get() + 1)
    });
    let original_request_frame = native_window
        .state
        .callbacks
        .request_frame
        .replace(Some(request_frame));
    let token = WPARAM(native_window.validation_number);
    let generation = LPARAM(native_window.registration.generation() as isize);

    for message in [
        WM_GPUI_FORCE_UPDATE_WINDOW,
        WM_GPUI_DISPLAY_TOPOLOGY_PUBLISHED,
    ] {
        assert_eq!(
            native_window.handle_gpui_events(
                native_window.hwnd,
                message,
                WPARAM(token.0 ^ 1),
                generation,
            ),
            None,
        );
        assert_eq!(
            native_window
                .handle_gpui_events(native_window.hwnd, message, token, LPARAM::default(),),
            None,
        );
        assert_eq!(
            native_window.handle_gpui_events(
                native_window.hwnd,
                message,
                token,
                LPARAM(generation.0.wrapping_add(1)),
            ),
            Some(0),
        );
    }
    assert_eq!(frame_requests.get(), 0);
    assert_eq!(
        unsafe {
            SendMessageW(
                native_window.hwnd,
                WM_GPUI_FORCE_UPDATE_WINDOW,
                Some(token),
                Some(generation),
            )
        }
        .0,
        0,
    );
    assert_eq!(frame_requests.get(), 1);
    native_window
        .state
        .callbacks
        .request_frame
        .set(original_request_frame);

    native_window.state.last_reported_modifiers.set(None);
    native_window
        .quiesce_interaction_for_test()
        .expect("native input test window should quiesce");
    assert_eq!(
        native_window.handle_keydown_msg(WPARAM(VK_SHIFT.0 as usize), LPARAM::default()),
        Some(0),
    );
    assert_eq!(native_window.state.last_reported_modifiers.get(), None);

    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("native input test window should retire");
    assert!(native_window.is_native_window_terminal());
    assert_eq!(
        native_window.handle_gpui_events(
            native_window.hwnd,
            WM_GPUI_FORCE_UPDATE_WINDOW,
            token,
            generation,
        ),
        Some(0),
    );
    assert_eq!(frame_requests.get(), 1);
    assert_eq!(
        native_window.handle_keydown_msg(WPARAM(VK_SHIFT.0 as usize), LPARAM::default()),
        Some(0),
    );
    assert_eq!(native_window.state.last_reported_modifiers.get(), None);
}
