use super::*;
use crate::dialog::show_dialog;
use open_gpui::WindowHandle;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use windows::Win32::System::Com::{APTTYPE_STA, CoGetApartmentType};

fn native_dialog_window() -> (
    Rc<WindowsPlatform>,
    Application,
    WindowHandle<Empty>,
    Rc<WindowsWindowInner>,
) {
    discard_stale_quit_messages();
    let platform = Rc::new(
        WindowsPlatform::new_windowed_for_test().expect("native dialog platform should initialize"),
    );
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
        .expect("native dialog owner should open");
    let hwnd = platform.raw_window_handles.read().last().unwrap().as_raw();
    let native = platform.window_from_hwnd(hwnd).unwrap();
    (platform, app, window, native)
}

#[test]
fn active_dialog_worker_defers_exact_native_retirement_and_cancels_queued_work() {
    let (platform, mut app, window, native) = native_dialog_window();
    let pump = Rc::downgrade(&platform.inner);
    let started = Arc::new(AtomicBool::new(false));
    let queued_started = Arc::new(AtomicBool::new(false));
    let (release, released) = mpsc::channel();
    let mut active = show_dialog(
        Some(native.dialog_owner.clone()),
        &platform.foreground_executor,
        {
            let started = started.clone();
            move |hwnd| {
                let mut apartment = Default::default();
                let mut qualifier = Default::default();
                unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }?;
                anyhow::ensure!(
                    apartment == APTTYPE_STA,
                    "native dialog worker must own an STA"
                );
                anyhow::ensure!(
                    unsafe { IsWindow(Some(hwnd)).as_bool() },
                    "owner must be live"
                );
                started.store(true, Ordering::Release);
                released.recv_timeout(Duration::from_secs(5))?;
                anyhow::ensure!(
                    unsafe { IsWindow(Some(hwnd)).as_bool() },
                    "owner retired before dialog exit"
                );
                Ok(())
            }
        },
    );
    pump_messages_until_with_delay(
        &pump,
        "dedicated native dialog worker starts",
        Duration::from_millis(1),
        || started.load(Ordering::Acquire),
    );
    let mut queued = show_dialog(
        Some(native.dialog_owner.clone()),
        &platform.foreground_executor,
        {
            let queued_started = queued_started.clone();
            move |_| {
                queued_started.store(true, Ordering::Release);
                Ok(())
            }
        },
    );
    platform.inner.run_foreground_task();
    assert!(!queued_started.load(Ordering::Acquire));
    platform.lifecycle_test_probe.clear_events();
    post_message(native.hwnd, WM_CLOSE, WPARAM::default(), LPARAM::default());
    pump_messages_until_with_delay(
        &pump,
        "dialog owner retirement retains its exact presentation ticket",
        Duration::from_millis(1),
        || {
            native.presentation_shutdown_claimed()
                && !unsafe { IsWindowVisible(native.hwnd).as_bool() }
        },
    );
    assert!(unsafe { IsWindow(Some(native.hwnd)).as_bool() });
    assert!(!unsafe { IsWindowVisible(native.hwnd).as_bool() });
    assert!(is_registered(&platform, native.hwnd));
    assert!(
        app.update_for_test(|cx| window.update(cx, |_, _, _| ()))
            .is_err()
    );
    let shutdown = native.presentation_shutdown_ticket();
    assert!(shutdown.snapshot().quiesced());
    assert!(
        platform.lifecycle_test_probe.events().iter().all(|event| {
            !matches!(event, NativeWindowLifecycleTestEvent::DestroyEntered { .. })
        })
    );
    assert!(active.try_recv().unwrap().is_none());
    let foreground_progress = Rc::new(Cell::new(false));
    platform
        .foreground_executor
        .spawn({
            let foreground_progress = foreground_progress.clone();
            async move { foreground_progress.set(true) }
        })
        .detach();
    platform.inner.run_foreground_task();
    assert!(foreground_progress.get());
    release.send(()).unwrap();
    pump_messages_until_with_delay(
        &pump,
        "dialog exit permits exact native destruction",
        Duration::from_millis(2),
        || native.is_native_window_terminal(),
    );
    pump_messages_until_idle(&pump, "dialog result and retirement settle");
    assert!(matches!(active.try_recv(), Ok(Some(Err(_)))));
    assert!(queued.try_recv().is_err());
    assert!(!queued_started.load(Ordering::Acquire));
    assert!(!is_registered(&platform, native.hwnd));
    assert!(
        platform
            .inner
            .native_retirement
            .borrow()
            .pending_windows
            .is_empty()
    );
    let expected = (window.window_id(), shutdown.snapshot().generation());
    let events = platform.lifecycle_test_probe.events();
    assert!(events.iter().any(|event| matches!(event,
        NativeWindowLifecycleTestEvent::DestroyEntered { window_id, generation }
        if (*window_id, *generation) == expected
    )));
    assert!(events.iter().any(|event| matches!(event,
        NativeWindowLifecycleTestEvent::NativeTerminal { window_id, generation }
        if (*window_id, *generation) == expected
    )));
    drop(app);
    drop(native);
    drop(platform);
    pump_messages_until_with_delay(
        &pump,
        "native dialog platform releases after retirement",
        Duration::from_millis(1),
        || pump.upgrade().is_none(),
    );
}

#[test]
fn dropped_dialog_receiver_keeps_serialization_until_worker_exit() {
    let (platform, mut app, window, native) = native_dialog_window();
    let pump = Rc::downgrade(&platform.inner);
    let started = Arc::new(AtomicBool::new(false));
    let next_started = Arc::new(AtomicBool::new(false));
    let (release, released) = mpsc::channel();
    let active = show_dialog(
        Some(native.dialog_owner.clone()),
        &platform.foreground_executor,
        {
            let started = started.clone();
            move |_| {
                started.store(true, Ordering::Release);
                released.recv_timeout(Duration::from_secs(5))?;
                Ok(())
            }
        },
    );
    pump_messages_until_with_delay(
        &pump,
        "cancellable dialog worker starts",
        Duration::from_millis(1),
        || started.load(Ordering::Acquire),
    );
    drop(active);
    let mut next = show_dialog(
        Some(native.dialog_owner.clone()),
        &platform.foreground_executor,
        {
            let next_started = next_started.clone();
            move |hwnd| {
                anyhow::ensure!(
                    unsafe { IsWindow(Some(hwnd)).as_bool() },
                    "next dialog owner must remain live"
                );
                next_started.store(true, Ordering::Release);
                Ok(())
            }
        },
    );
    platform.inner.run_foreground_task();
    assert!(!next_started.load(Ordering::Acquire));
    assert!(native.dialog_owner.try_lock_idle().is_none());
    release.send(()).unwrap();
    let mut result = None;
    pump_messages_until_with_delay(
        &pump,
        "receiver cancellation releases only its dialog",
        Duration::from_millis(1),
        || {
            result = next.try_recv().unwrap();
            result.is_some()
        },
    );
    result.unwrap().unwrap();
    assert!(next_started.load(Ordering::Acquire));
    assert!(unsafe { IsWindow(Some(native.hwnd)).as_bool() });
    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
    });
    pump_messages_until_with_delay(
        &pump,
        "cancelled-dialog owner tears down normally",
        Duration::from_millis(1),
        || native.is_native_window_terminal(),
    );
    drop(app);
    drop(native);
    drop(platform);
    pump_messages_until_with_delay(
        &pump,
        "cancelled-dialog platform releases",
        Duration::from_millis(1),
        || pump.upgrade().is_none(),
    );
}

#[test]
fn native_accelerators_route_only_registered_windows_and_recover_callback_panics() {
    use windows::Win32::UI::{Input::KeyboardAndMouse::VK_F1, WindowsAndMessaging::GWLP_USERDATA};

    let (platform, mut app, window, native) = native_dialog_window();
    let pump = Rc::downgrade(&platform.inner);
    let foreign = unsafe {
        CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("foreign native keyboard target"),
            Default::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
    }
    .expect("foreign host window should open");
    unsafe { crate::set_window_long(foreign, GWLP_USERDATA, 1) };
    let mut message = MSG {
        hwnd: foreign,
        message: WM_KEYDOWN,
        wParam: WPARAM(VK_F1.0 as usize),
        lParam: LPARAM(1),
        ..Default::default()
    };
    assert!(!platform.inner.translate_accelerator(&message));
    message.message = WM_SYSKEYDOWN;
    assert!(!platform.inner.translate_accelerator(&message));
    unsafe { DestroyWindow(foreign) }.unwrap();

    let calls = Rc::new(Cell::new(0usize));
    let consume = Rc::new(Cell::new(true));
    let panic_once = Rc::new(Cell::new(false));
    let _subscription = app.update_for_test(|cx| {
        window
            .update(cx, |_, window, _| {
                window.intercept_window_key_down({
                    let calls = calls.clone();
                    let consume = consume.clone();
                    let panic_once = panic_once.clone();
                    move |_, window, cx| {
                        calls.set(calls.get() + 1);
                        assert!(
                            !panic_once.replace(false),
                            "injected accelerator callback panic"
                        );
                        if consume.get() {
                            cx.stop_propagation();
                            window.prevent_default();
                        }
                    }
                })
            })
            .unwrap()
    });
    message.hwnd = native.hwnd;
    message.message = WM_KEYDOWN;
    assert!(platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 1);
    consume.set(false);
    assert!(!platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 2);
    panic_once.set(true);
    assert!(platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 3);
    consume.set(true);
    assert!(platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 4);
    native.quiesce_interaction_for_test().unwrap();
    assert!(platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 4);
    app.update_for_test(|cx| {
        window
            .update(cx, |_, window, cx| window.remove_window(cx))
            .unwrap();
    });
    pump_messages_until_with_delay(
        &pump,
        "accelerator target retires",
        Duration::from_millis(1),
        || native.is_native_window_terminal(),
    );
    assert!(!platform.inner.translate_accelerator(&message));
    assert_eq!(calls.get(), 4);
    drop(app);
    drop(native);
    drop(platform);
    pump_messages_until_with_delay(
        &pump,
        "accelerator platform releases",
        Duration::from_millis(1),
        || pump.upgrade().is_none(),
    );
}
