use super::{
    WM_GPUI_DISPLAY_TOPOLOGY_PUBLISHED, WM_GPUI_FORCE_UPDATE_WINDOW, WM_GPUI_GPU_DEVICE_LOST,
};
use crate::{WindowsPlatform, WindowsWindowInner};
use open_gpui::{
    AppContext as _, Application, Empty, GraphicalEnvironment, MouseButton, Platform as _,
    QuitMode, RequestFrameOptions, WindowBounds, WindowHandle, WindowMouseEvent, WindowOptions,
    WindowingRequest, px, size,
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
        Input::KeyboardAndMouse::VK_SHIFT,
        WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowPos,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
        },
    },
};

fn native_input_window() -> (Application, WindowHandle<Empty>, Rc<WindowsWindowInner>) {
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
    (app, window, native_window)
}

#[test]
fn stationary_mouse_moves_preserve_reentry_capture_and_exact_physical_updates() {
    let (mut app, window, native_window) = native_input_window();
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
fn private_input_messages_require_validation_and_the_current_native_generation() {
    let (mut app, window, native_window) = native_input_window();
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
        WM_GPUI_GPU_DEVICE_LOST,
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
