use super::*;
use crate::WM_GPUI_END_SESSION;
use std::{
    io::Write as _,
    process::{Command, Stdio},
};
use windows::Win32::UI::WindowsAndMessaging::{WM_ENDSESSION, WM_QUERYENDSESSION};

#[test]
fn native_session_quit_preserves_registered_authority_and_terminal_fence() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    platform.connect_initially();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                show: false,
                focus_on_appearing: false,
                ..WindowOptions::default()
            },
            |_, cx| cx.new(|_| Empty),
        )
        .unwrap();
    });
    let registration = *platform.raw_window_handles.read().last().unwrap();
    let hwnd = registration.as_raw();
    let calls = Rc::new(Cell::new(0));
    platform.on_quit(Box::new({
        let calls = calls.clone();
        move || {
            calls.set(calls.get() + 1);
            false
        }
    }));

    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_QUERYENDSESSION, None, None) }.0,
        1
    );
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_ENDSESSION, None, None) }.0,
        0
    );
    unsafe {
        SendMessageW(
            platform.handle,
            WM_GPUI_END_SESSION,
            Some(WPARAM(platform.inner.validation_number.wrapping_add(1))),
            None,
        );
    }

    let replacement = RegisteredWindow::new(
        hwnd,
        registration.generation().checked_add(1).unwrap(),
        registration.window_id(),
    );
    let index = platform
        .raw_window_handles
        .read()
        .iter()
        .position(|entry| entry.matches(registration))
        .unwrap();
    platform.raw_window_handles.write()[index] = replacement;
    unsafe { SendMessageW(hwnd, WM_ENDSESSION, Some(WPARAM(1)), None) };
    platform.raw_window_handles.write()[index] = registration;
    assert_eq!(calls.get(), 0);
    let mut message = MSG::default();
    assert!(!unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() });

    unsafe { SendMessageW(hwnd, WM_ENDSESSION, Some(WPARAM(1)), None) };
    assert_eq!(calls.get(), 1);
    let mut quit_posted = false;
    let mut dispatched = Vec::new();
    for _ in 0..MAX_MESSAGE_PUMP_ATTEMPTS {
        if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
            std::thread::yield_now();
            continue;
        }
        if message.message == WM_QUIT {
            quit_posted = true;
            break;
        }
        dispatched.push(message.message);
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    assert!(
        quit_posted,
        "session quit was not received: {dispatched:x?}"
    );
    assert!(!platform.terminal_shutdown_complete.get());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });

    drop(app);
    drop(platform);
    discard_stale_quit_messages();
}

struct SessionExitLogger;

impl log::Log for SessionExitLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        false
    }

    fn log(&self, _: &log::Record<'_>) {}

    fn flush(&self) {
        println!("native-session-log-flushed");
        std::io::stdout().flush().unwrap();
    }
}

#[test]
fn native_session_quit_flushes_and_uses_production_exit() {
    const CHILD: &str = "OPEN_GPUI_NATIVE_SESSION_EXIT_CHILD";
    if std::env::var_os(CHILD).is_some() {
        static LOGGER: SessionExitLogger = SessionExitLogger;
        log::set_logger(&LOGGER).unwrap();
        let platform = WindowsPlatform::new(true).unwrap();
        platform.on_quit(Box::new(|| {
            println!("native-session-shutdown-completed");
            true
        }));
        unsafe { SendMessageW(platform.handle, WM_ENDSESSION, Some(WPARAM(1)), None) };
        panic!("completed production session shutdown returned instead of exiting");
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform::native_test_support::native_quit_tests::native_session_quit_flushes_and_uses_production_exit",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("isolated session child timed out: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("native-session-shutdown-completed"),
        "{stdout}"
    );
    assert!(stdout.contains("native-session-log-flushed"), "{stdout}");
}
