use super::Application;
use crate::{Empty, TestAppContext, TestPlatform, px, size};
use std::{cell::Cell, rc::Rc};

#[crate::test]
fn native_quit_callback_queues_behind_a_busy_app_and_reports_actual_completion(
    cx: &mut TestAppContext,
) {
    let mut quit = cx
        .test_platform()
        .take_quit_callback()
        .expect("App must register its actual native quit callback");
    let observed = Rc::new(Cell::new(0));
    let _subscription = cx.read(|app| {
        let observed = observed.clone();
        app.on_app_quit(move |_| {
            observed.set(observed.get() + 1);
            std::future::ready(())
        })
    });

    cx.update(|_| {
        assert!(!quit(), "a queued Quit is not synchronous completion");
        assert_eq!(observed.get(), 0);
    });
    cx.run_until_parked();

    assert_eq!(observed.get(), 1);
    assert!(cx.test_platform().did_quit());
    assert!(quit());
    assert!(quit());
    assert_eq!(observed.get(), 1);
}

#[crate::test]
fn native_quit_callback_waits_for_the_exact_native_retirement(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
    let terminal = cx.hold_window_native_terminal(window.into());
    let mut quit = cx
        .test_platform()
        .take_quit_callback()
        .expect("App must register its actual native quit callback");

    assert!(!quit());
    cx.run_until_parked();
    assert!(!quit(), "a retired registry is not native completion");
    assert!(!cx.test_platform().did_quit());

    assert!(terminal.release());
    cx.run_until_parked();

    assert!(quit());
    assert!(cx.test_platform().did_quit());
}

#[crate::test]
fn native_quit_callback_accepts_a_missing_app(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    let application = Application::with_platform(platform.clone());
    let mut quit = platform.take_quit_callback().unwrap();
    drop(application);

    assert!(quit());
    assert!(platform.did_quit());
}
