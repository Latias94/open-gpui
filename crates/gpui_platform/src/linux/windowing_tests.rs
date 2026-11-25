use super::*;
use open_gpui::{
    Context, IntoElement, Render, Window, WindowActivationPolicy, WindowBounds, WindowHandle,
    WindowId, point, px, size,
};
use std::time::{Duration, Instant};

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        open_gpui::Empty
    }
}

fn platform() -> LinuxPlatform {
    let platform =
        LinuxPlatform::new_with_startup(WindowingModes::all(), StartupEnvironment::default());
    platform.set_initial_windowing(WindowingRequest::Headless);
    platform
}

fn params() -> WindowParams {
    let bounds = open_gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    WindowParams {
        bounds,
        window_bounds: WindowBounds::Windowed(bounds),
        titlebar: None,
        kind: WindowKind::Normal,
        is_movable: true,
        app_owns_titlebar_drag: false,
        is_resizable: true,
        is_minimizable: true,
        accepts_pointer_input: true,
        focus_on_appearing: false,
        activation_policy: WindowActivationPolicy::default(),
        transient_for: None,
        show: false,
        provisional_session: None,
        icon: None,
        display_id: None,
        window_min_size: None,
    }
}

fn handle() -> AnyWindowHandle {
    WindowHandle::<Blank>::new(WindowId::from(1)).into()
}

#[test]
fn headless_rejects_anchored_popup_before_publication() {
    let platform = platform();
    let mut params = params();
    params.kind = WindowKind::AnchoredPopup(open_gpui::popup::PopupOptions {
        parent: handle(),
        anchor_rect: params.bounds,
        anchor: Default::default(),
        gravity: Default::default(),
        constraint_adjustment: Default::default(),
        offset: Default::default(),
        grab: false,
    });
    let error = platform.validate_window_kind(&params.kind).unwrap_err();
    assert!(error.is::<open_gpui::popup::PopupNotSupportedError>());
    assert!(!platform.connection.borrow().has_windows());
    let Err(error) = platform.open_window(handle(), params) else {
        panic!("headless must reject native popups");
    };
    assert!(error.is::<open_gpui::popup::PopupNotSupportedError>());
    assert!(!platform.connection.borrow().has_windows());
}

#[test]
fn headless_window_kind_validation_preserves_ordinary_window_creation() {
    let platform = platform();
    for kind in [
        WindowKind::Normal,
        WindowKind::Floating,
        WindowKind::Dialog,
        WindowKind::PopUp,
    ] {
        assert!(platform.validate_window_kind(&kind).is_ok());
    }
    assert!(!platform.connection.borrow().has_windows());
}

#[cfg(any(feature = "wayland", feature = "x11"))]
fn missing_display() -> WindowingRequest {
    WindowingRequest::Windowed(GraphicalEnvironment {
        #[cfg(feature = "wayland")]
        wayland_display: Some("/gpui-test/missing-windowing-socket".into()),
        #[cfg(all(feature = "x11", not(feature = "wayland")))]
        x11_display: Some(":65535".into()),
        ..GraphicalEnvironment::default()
    })
}

fn finish(executor: &BackgroundExecutor, task: Task<anyhow::Result<()>>) -> anyhow::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    executor
        .spawn(async move {
            sender.send(task.await).ok();
        })
        .detach();
    receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("windowing waiter was not settled")
}

fn assert_error(result: anyhow::Result<()>, expected: &str) {
    let error = result.expect_err("the request must be refused");
    assert!(
        format!("{error:#}").contains(expected),
        "expected {expected:?}, got {error:#}"
    );
}

#[test]
fn compiled_backend_masks_and_headless_startup_fallback_are_enforced() {
    let environment = GraphicalEnvironment {
        wayland_display: Some("wayland-test".into()),
        x11_display: Some(":99".into()),
        ..GraphicalEnvironment::default()
    };
    assert!(select_backend(WindowingModes::HEADLESS, &environment).is_none());
    assert_eq!(
        select_backend(WindowingModes::WAYLAND, &environment).is_some(),
        cfg!(feature = "wayland")
    );
    assert_eq!(
        select_backend(WindowingModes::X11, &environment).is_some(),
        cfg!(feature = "x11")
    );
    #[cfg(feature = "wayland")]
    assert!(matches!(
        select_backend(WindowingModes::all(), &environment),
        Some(Backend::Wayland)
    ));
    let platform = platform();
    platform.set_initial_windowing(WindowingRequest::Windowed(GraphicalEnvironment::default()));
    platform.connect_initially();
    assert_eq!(platform.compositor_name(), "headless");
    assert!(platform.displays().is_empty());

    let platform =
        LinuxPlatform::new_with_startup(WindowingModes::empty(), StartupEnvironment::default());
    assert_error(
        finish(
            &platform.background_executor(),
            platform.request_windowing(WindowingRequest::Headless),
        ),
        "does not allow headless",
    );
    platform.set_initial_windowing(WindowingRequest::Windowed(GraphicalEnvironment::default()));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| platform.connect_initially()))
            .is_err()
    );
}

#[test]
fn admission_refusal_and_unwind_release_the_validator_and_transition_slot() {
    let platform = platform();
    let task = platform.request_windowing_with_validation(
        WindowingRequest::Headless,
        Box::new(|| anyhow::bail!("Core owner is busy")),
    );
    assert_error(
        finish(&platform.background_executor(), task),
        "Core owner is busy",
    );
    assert!(!platform.transition_in_progress.get());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        platform.request_windowing_with_validation(
            WindowingRequest::Headless,
            Box::new(|| panic!("validator unwind")),
        )
    }));
    assert!(result.is_err());
    assert!(!platform.transition_in_progress.get());
    assert!(platform.pending_mode.borrow().is_none());
    assert_error(
        finish(
            &platform.background_executor(),
            platform.request_windowing(WindowingRequest::Headless),
        ),
        "already headless",
    );
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn dropped_waiter_keeps_admitted_work_and_duplicate_requests_are_refused() {
    let platform = platform();
    let retained = Rc::new(());
    let weak = Rc::downgrade(&retained);
    let task = platform.request_windowing_with_validation(
        missing_display(),
        Box::new(move || {
            let _ = &retained;
            Ok(())
        }),
    );
    drop(task);
    assert!(weak.upgrade().is_some());
    assert_error(
        finish(
            &platform.background_executor(),
            platform.request_windowing(missing_display()),
        ),
        "already in progress",
    );
    platform.apply_pending_mode();
    assert!(weak.upgrade().is_none());
    assert!(platform.pending_mode.borrow().is_none());
    let retry = platform.request_windowing(missing_display());
    assert!(platform.pending_mode.borrow().is_some());
    platform.apply_pending_mode();
    assert_error(
        finish(&platform.background_executor(), retry),
        "failed to connect",
    );
    assert_eq!(platform.compositor_name(), "headless");
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn a_window_opened_after_admission_refuses_apply_and_allows_retry_after_drop() {
    let platform = platform();
    let text_system = platform.text_system();
    let task = platform.request_windowing(missing_display());
    let window = platform.open_window(handle(), params()).unwrap();
    platform.apply_pending_mode();
    assert_error(
        finish(&platform.background_executor(), task),
        "while windows are open",
    );
    assert_eq!(platform.compositor_name(), "headless");
    assert!(Arc::ptr_eq(&text_system, &platform.text_system()));
    drop(window);
    let retry = platform.request_windowing(missing_display());
    platform.apply_pending_mode();
    assert_error(
        finish(&platform.background_executor(), retry),
        "failed to connect",
    );
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn late_owner_refusal_and_validation_unwind_preserve_the_connection() {
    let platform = platform();
    let calls = Rc::new(Cell::new(0));
    let ready = Rc::new(Cell::new(true));
    let task = platform.request_windowing_with_validation(
        missing_display(),
        Box::new({
            let calls = calls.clone();
            let ready = ready.clone();
            move || {
                calls.set(calls.get() + 1);
                anyhow::ensure!(ready.get(), "native retirement is pending");
                Ok(())
            }
        }),
    );
    assert_eq!(calls.get(), 1);
    ready.set(false);
    platform.apply_pending_mode();
    assert_eq!(calls.get(), 2);
    assert_error(
        finish(&platform.background_executor(), task),
        "native retirement is pending",
    );
    let calls = Cell::new(0);
    let task = platform.request_windowing_with_validation(
        missing_display(),
        Box::new(move || {
            calls.set(calls.get() + 1);
            assert_eq!(calls.get(), 1, "late validation panic");
            Ok(())
        }),
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || platform.apply_pending_mode()
        ))
        .is_err()
    );
    assert_error(finish(&platform.background_executor(), task), "canceled");
    assert!(!platform.transition_in_progress.get());
    assert!(platform.graphical_environment().is_none());
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn quit_during_launch_and_drop_settle_waiters_without_another_dispatch() {
    let platform = Rc::new(platform());
    let task = platform.request_windowing(missing_display());
    let quits = Rc::new(Cell::new(0));
    platform.on_quit(Box::new({
        let quits = quits.clone();
        move || {
            quits.set(quits.get() + 1);
            true
        }
    }));
    platform.run(Box::new({
        let platform = platform.clone();
        move || platform.quit()
    }));
    assert_eq!(quits.get(), 1);
    assert_error(finish(&platform.background_executor(), task), "shutdown");
    assert_error(
        finish(
            &platform.background_executor(),
            platform.request_windowing(missing_display()),
        ),
        "shutdown",
    );
    assert!(platform.open_window(handle(), params()).is_err());

    let platform = self::platform();
    let executor = platform.background_executor();
    let task = platform.request_windowing(missing_display());
    drop(platform);
    assert_error(finish(&executor, task), "shutdown");
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn quit_from_late_validation_refuses_apply_and_reentrant_admission() {
    let platform = Rc::new(platform());
    let calls = Cell::new(0);
    let task = platform.request_windowing_with_validation(
        missing_display(),
        Box::new({
            let weak = Rc::downgrade(&platform);
            move || {
                let platform = weak.upgrade().unwrap();
                calls.set(calls.get() + 1);
                if calls.get() > 1 {
                    let duplicate = platform.request_windowing(missing_display());
                    assert_error(
                        finish(&platform.background_executor(), duplicate),
                        "already in progress",
                    );
                    platform.quit();
                }
                Ok(())
            }
        }),
    );
    platform.apply_pending_mode();
    assert_error(finish(&platform.background_executor(), task), "shutdown");
    assert!(platform.graphical_environment().is_none());
    assert!(!platform.transition_in_progress.get());
}

#[test]
fn connection_notifications_restore_observers_after_reentry_and_unwind() {
    let platform = Rc::new(platform());
    let displays = Rc::new(Cell::new(0));
    platform.on_displays_changed(Box::new({
        let displays = displays.clone();
        let weak = Rc::downgrade(&platform);
        move || {
            assert!(weak.upgrade().unwrap().displays().is_empty());
            displays.set(displays.get() + 1);
        }
    }));
    platform.on_keyboard_layout_change(Box::new(|| panic!("keyboard observer unwind")));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || platform.notify_connection_changed()
        ))
        .is_err()
    );
    assert_eq!(displays.get(), 1);
    assert!(
        platform
            .common
            .borrow()
            .callbacks
            .keyboard_layout_change
            .is_some()
    );
    let replaced = Rc::new(Cell::new(0));
    platform.on_keyboard_layout_change(Box::new({
        let weak = Rc::downgrade(&platform);
        let replaced = replaced.clone();
        move || {
            weak.upgrade().unwrap().on_keyboard_layout_change(Box::new({
                let replaced = replaced.clone();
                move || replaced.set(replaced.get() + 1)
            }))
        }
    }));
    platform.notify_connection_changed();
    platform.notify_connection_changed();
    assert_eq!(replaced.get(), 1);
    assert_eq!(displays.get(), 3);
}

#[test]
fn connection_notifications_refresh_power_cache_without_exposing_headless_power() {
    let platform = Rc::new(platform());
    platform.common.borrow_mut().power_mode = PowerMode::LowPower;
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    platform.on_power_mode_change(Arc::new(move || {
        sender.send(()).unwrap();
    }));
    platform.notify_connection_changed();
    assert_eq!(receiver.try_iter().count(), 1);
    assert_eq!(platform.power_mode(), PowerMode::Unknown);
    assert_eq!(platform.common.borrow().power_mode, PowerMode::LowPower);
    #[cfg(any(feature = "wayland", feature = "x11"))]
    assert!(platform.common.borrow().power_mode_listener.is_none());
}

#[cfg(any(feature = "wayland", feature = "x11"))]
#[test]
fn pre_run_wakeup_applies_on_first_dispatch_and_idle_backlog_keeps_progressing() {
    let platform = platform();
    let task = platform.request_windowing(missing_display());
    let mut event_loop = platform.event_loop.borrow_mut().take().unwrap();
    let handle = event_loop.handle();
    let timeout = handle
        .insert_source(
            calloop::timer::Timer::from_duration(Duration::from_secs(2)),
            |_, _, _| calloop::timer::TimeoutAction::Drop,
        )
        .unwrap();
    let start = Instant::now();
    event_loop.dispatch(None, &mut ()).unwrap();
    platform.apply_pending_mode();
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "the pre-run wake was lost"
    );
    assert_error(
        finish(&platform.background_executor(), task),
        "failed to connect",
    );
    let task = platform.request_windowing(missing_display());
    let count = Rc::new(Cell::new(0));
    let reentrant_idle = Rc::new(Cell::new(false));
    handle.insert_idle({
        let handle = handle.clone();
        let reentrant_idle = reentrant_idle.clone();
        move |_| {
            handle.insert_idle(move |_| reentrant_idle.set(true));
        }
    });
    for _ in 0..40 {
        let count = count.clone();
        handle.insert_idle(move |_| {
            std::thread::sleep(Duration::from_millis(1));
            count.set(count.get() + 1);
        });
    }
    event_loop.dispatch(None, &mut ()).unwrap();
    platform.apply_pending_mode();
    assert!(
        count.get() > 0 && count.get() < 40,
        "idle backlog must yield to physical mode application"
    );
    assert_error(
        finish(&platform.background_executor(), task),
        "failed to connect",
    );
    while count.get() < 40 || !reentrant_idle.get() {
        event_loop.dispatch(None, &mut ()).unwrap();
    }
    assert!(reentrant_idle.get());
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "queued idle callbacks must force a nonblocking poll"
    );
    handle.remove(timeout);
}

#[test]
fn replacing_and_closing_connection_sources_preserves_common_dispatch() {
    let platform = platform();
    let mut event_loop = platform.event_loop.borrow_mut().take().unwrap();
    let handle = event_loop.handle();
    let old_events = Rc::new(Cell::new(0));
    let (old_sender, old_source) = calloop::channel::channel();
    let old_token = handle
        .insert_source(old_source, {
            let old_events = old_events.clone();
            move |_: calloop::channel::Event<()>, _, _| old_events.set(old_events.get() + 1)
        })
        .unwrap();
    old_sender.send(()).unwrap();
    handle.remove(old_token);
    let new_events = Rc::new(Cell::new(0));
    let closed = Rc::new(Cell::new(0));
    let (sender, source) = calloop::channel::channel();
    handle
        .insert_source(source, {
            let new_events = new_events.clone();
            let closed = closed.clone();
            move |event, _, _| match event {
                calloop::channel::Event::Msg(()) => new_events.set(new_events.get() + 1),
                calloop::channel::Event::Closed => closed.set(closed.get() + 1),
            }
        })
        .unwrap();
    sender.send(()).unwrap();
    drop(sender);
    let foreground = Rc::new(Cell::new(0));
    platform
        .foreground_executor()
        .spawn({
            let foreground = foreground.clone();
            async move { foreground.set(foreground.get() + 1) }
        })
        .detach();
    for _ in 0..3 {
        event_loop.dispatch(Duration::ZERO, &mut ()).unwrap();
    }
    assert_eq!(old_events.get(), 0);
    assert_eq!(new_events.get(), 1);
    assert_eq!(closed.get(), 1);
    assert_eq!(foreground.get(), 1);
    let task = platform.foreground_executor().spawn({
        let foreground = foreground.clone();
        async move { foreground.set(foreground.get() + 1) }
    });
    for _ in 0..3 {
        event_loop.dispatch(Duration::ZERO, &mut ()).unwrap();
    }
    assert!(task.is_ready());
    assert_eq!(foreground.get(), 2);
    drop(old_sender);
}

#[test]
fn platform_drop_removes_common_sources_and_releases_the_shared_owner() {
    let platform = platform();
    let common = Rc::downgrade(&platform.common);
    let handle = platform.loop_handle.clone();
    drop(platform);
    assert!(
        common.upgrade().is_none(),
        "a registered foreground callback must not retain the common runtime through its LoopHandle"
    );
    drop(handle);
}

struct ForegroundDrop(Rc<RefCell<Vec<std::thread::ThreadId>>>);

impl Drop for ForegroundDrop {
    fn drop(&mut self) {
        self.0.borrow_mut().push(std::thread::current().id());
    }
}

struct FinishOnDrop {
    platform: Rc<LinuxPlatform>,
    dropped: Rc<Cell<usize>>,
}

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
        self.platform.finish_runtime();
    }
}

#[test]
fn run_exit_cancels_a_new_foreground_task_retaining_the_platform_on_its_main_thread() {
    struct RemoveSourceOnDrop {
        handle: LoopHandle<'static, ()>,
        token: RegistrationToken,
    }

    impl Drop for RemoveSourceOnDrop {
        fn drop(&mut self) {
            self.handle.remove(self.token);
        }
    }

    let platform = Rc::new(platform());
    let weak = Rc::downgrade(&platform);
    let common = Rc::downgrade(&platform.common);
    let loop_handle = platform.loop_handle.clone();
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let ran = Rc::new(Cell::new(false));
    let capture = ForegroundDrop(dropped.clone());
    let (_sender, source) = calloop::channel::channel::<()>();
    let token = loop_handle.insert_source(source, |_, _, _| {}).unwrap();
    let remove_source = RemoveSourceOnDrop {
        handle: loop_handle.clone(),
        token,
    };
    platform
        .foreground_executor()
        .spawn({
            let platform = platform.clone();
            let ran = ran.clone();
            async move {
                platform
                    .foreground_executor()
                    .spawn({
                        let platform = platform.clone();
                        async move {
                            let _retained = (platform, capture, remove_source);
                            ran.set(true);
                        }
                    })
                    .detach();
                platform.quit();
            }
        })
        .detach();
    platform.run(Box::new(|| {}));
    assert!(
        !ran.get(),
        "a task queued by the quitting runnable must not be polled"
    );
    assert_eq!(&*dropped.borrow(), &[std::thread::current().id()]);
    assert!(platform.common_sources.borrow().is_empty());
    assert!(platform.common.borrow().foreground_idles.is_empty());
    assert!(
        loop_handle.enable(&token).is_err(),
        "task captures must be dropped outside calloop's source borrow"
    );

    platform
        .foreground_executor()
        .spawn({
            let platform = platform.clone();
            let ran = ran.clone();
            let capture = ForegroundDrop(dropped.clone());
            async move {
                let _retained = (platform, capture);
                ran.set(true);
            }
        })
        .detach();
    assert!(
        !ran.get(),
        "closed-source admission must cancel on the owning thread"
    );
    assert_eq!(&*dropped.borrow(), &[std::thread::current().id(); 2]);
    drop(platform);
    assert!(
        weak.upgrade().is_none(),
        "the pending queue must not retain the platform"
    );
    assert!(
        common.upgrade().is_none(),
        "a retained LoopHandle must not retain the closed runtime"
    );
    drop(loop_handle);
}

#[test]
fn runnable_unwind_cancels_queued_and_idle_foreground_captures_on_the_main_thread() {
    let platform = Rc::new(platform());
    let weak = Rc::downgrade(&platform);
    let common = Rc::downgrade(&platform.common);
    let loop_handle = platform.loop_handle.clone();
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let ran = Rc::new(Cell::new(false));
    let capture = ForegroundDrop(dropped.clone());
    let reentered = Rc::new(Cell::new(0));
    platform.common.borrow_mut().after_runnable = Some(Rc::new(|| panic!("after runnable unwind")));
    platform
        .foreground_executor()
        .spawn({
            let platform = platform.clone();
            let ran = ran.clone();
            async move {
                platform
                    .foreground_executor()
                    .spawn({
                        let platform = platform.clone();
                        async move {
                            let _retained = (platform, capture);
                            ran.set(true);
                        }
                    })
                    .detach();
            }
        })
        .detach();
    platform
        .foreground_executor()
        .spawn({
            let platform = platform.clone();
            let ran = ran.clone();
            let capture = ForegroundDrop(dropped.clone());
            let finish = FinishOnDrop {
                platform: platform.clone(),
                dropped: reentered.clone(),
            };
            async move {
                let _retained = (platform, capture, finish);
                ran.set(true);
            }
        })
        .detach();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        platform.run(Box::new(|| {}));
    }))
    .expect_err("the real idle callback must unwind through the run boundary");
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"after runnable unwind"));
    assert!(
        !ran.get(),
        "neither queued nor pending idle work may run during unwind"
    );
    assert_eq!(&*dropped.borrow(), &[std::thread::current().id(); 2]);
    assert_eq!(
        reentered.get(),
        1,
        "canceling a pending idle must allow exit cleanup reentry"
    );
    assert!(platform.common_sources.borrow().is_empty());
    assert!(platform.common.borrow().foreground_idles.is_empty());
    drop(platform);
    assert!(weak.upgrade().is_none());
    assert!(common.upgrade().is_none());
    drop(loop_handle);
}

#[test]
fn launch_unwind_cancels_foreground_captures_before_the_event_loop_is_taken() {
    let platform = Rc::new(platform());
    let weak = Rc::downgrade(&platform);
    let common = Rc::downgrade(&platform.common);
    let loop_handle = platform.loop_handle.clone();
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let ran = Rc::new(Cell::new(false));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        platform.run(Box::new({
            let platform = platform.clone();
            let ran = ran.clone();
            let capture = ForegroundDrop(dropped.clone());
            move || {
                platform
                    .foreground_executor()
                    .spawn({
                        let platform = platform.clone();
                        async move {
                            let _retained = (platform, capture);
                            ran.set(true);
                        }
                    })
                    .detach();
                panic!("launch unwind");
            }
        }));
    }));
    assert!(result.is_err());
    assert!(!ran.get());
    assert_eq!(&*dropped.borrow(), &[std::thread::current().id()]);
    assert!(platform.event_loop.borrow().is_some());
    assert!(platform.common_sources.borrow().is_empty());
    drop(platform);
    assert!(weak.upgrade().is_none());
    assert!(common.upgrade().is_none());
    drop(loop_handle);
}

#[test]
fn run_exit_and_launch_unwind_release_timers_that_retain_the_dispatcher() {
    for unwind in [false, true] {
        let platform = Rc::new(platform());
        let dispatcher = Arc::downgrade(&platform.common.borrow().dispatcher);
        let timer = platform
            .background_executor()
            .timer(Duration::from_secs(3600));
        let (sender, receiver) = std::sync::mpsc::channel();
        platform
            .background_executor()
            .spawn(async move {
                timer.await;
                sender.send(()).ok();
            })
            .detach();
        assert_eq!(
            receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            platform.run(Box::new({
                let platform = platform.clone();
                move || {
                    if unwind {
                        panic!("timer owner launch unwind");
                    }
                    platform.quit();
                }
            }));
        }));
        assert_eq!(result.is_err(), unwind);
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("run shutdown must drop the real timer runnable before its deadline");
        drop(platform);
        let deadline = Instant::now() + Duration::from_secs(5);
        while dispatcher.upgrade().is_some() {
            assert!(
                Instant::now() < deadline,
                "pending timers must not retain the dispatcher"
            );
            std::thread::yield_now();
        }
    }
}

#[test]
fn background_executor_keeps_real_timer_deadlines_after_an_unlaunched_platform_drops() {
    let executor = {
        let platform = platform();
        platform.background_executor()
    };
    let duration = Duration::from_millis(80);
    let started = Instant::now();
    let task = executor.timer(duration);
    let (sender, receiver) = std::sync::mpsc::channel();
    executor
        .spawn(async move {
            task.await;
            sender.send(Instant::now()).unwrap();
        })
        .detach();
    let completed = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the standalone background executor's real timer must complete");
    assert!(
        completed.duration_since(started) >= duration,
        "dropping a temporary, never-launched platform must not cancel a standalone timer"
    );
}

#[test]
fn run_exit_keeps_the_connection_while_native_window_leases_are_retained() {
    let platform = Rc::new(platform());
    let window = platform.open_window(handle(), params()).unwrap();
    platform.run(Box::new({
        let platform = platform.clone();
        move || platform.quit()
    }));
    platform.finish_runtime();
    platform.finish_runtime();
    assert!(platform.connection.borrow().has_windows());
    assert_error(
        platform.validate_no_windows(),
        "while windows are open or retiring",
    );
    drop(window);
    assert!(!platform.connection.borrow().has_windows());
    assert!(platform.validate_no_windows().is_ok());
    platform.finish_runtime();
    assert!(platform.common_sources.borrow().is_empty());
    assert!(platform.common.borrow().foreground_idles.is_empty());
}

#[test]
fn canceled_foreground_capture_can_reenter_the_idempotent_run_exit_boundary() {
    let platform = Rc::new(platform());
    let weak = Rc::downgrade(&platform);
    let common = Rc::downgrade(&platform.common);
    let loop_handle = platform.loop_handle.clone();
    let dropped = Rc::new(Cell::new(0));
    platform.run(Box::new({
        let platform = platform.clone();
        let capture = FinishOnDrop {
            platform: platform.clone(),
            dropped: dropped.clone(),
        };
        move || {
            platform
                .foreground_executor()
                .spawn(async move {
                    let _capture = capture;
                    panic!("canceled foreground work must never run");
                })
                .detach();
            platform.quit();
        }
    }));
    assert_eq!(dropped.get(), 1);
    platform.finish_runtime();
    platform.run(Box::new(|| {
        panic!("a stopped platform must not launch again")
    }));
    assert_eq!(dropped.get(), 1);
    assert!(platform.common_sources.borrow().is_empty());
    assert!(platform.common.borrow().foreground_idles.is_empty());
    drop(platform);
    assert!(weak.upgrade().is_none());
    assert!(common.upgrade().is_none());
    drop(loop_handle);
}

#[cfg(feature = "wayland")]
#[test]
fn fixed_headless_and_headless_fallback_ignore_an_unclaimed_startup_socket() {
    for (allowed, initial) in [
        (WindowingModes::HEADLESS, None),
        (
            WindowingModes::all(),
            Some(WindowingRequest::Windowed(GraphicalEnvironment::default())),
        ),
    ] {
        let platform = Rc::new(LinuxPlatform::new_with_startup(
            allowed,
            StartupEnvironment {
                unclaimed_socket: true,
                ..StartupEnvironment::default()
            },
        ));
        if let Some(initial) = initial {
            platform.set_initial_windowing(initial);
        }
        let launched = Rc::new(Cell::new(false));
        platform.run(Box::new({
            let platform = platform.clone();
            let launched = launched.clone();
            move || {
                assert_eq!(platform.compositor_name(), "headless");
                assert!(platform.graphical_environment().is_none());
                launched.set(true);
                platform.quit();
            }
        }));
        assert!(launched.get());
        assert!(platform.startup_environment.borrow().is_none());
    }
}

#[cfg(feature = "wayland")]
#[test]
fn selecting_wayland_still_refuses_an_unclaimed_startup_socket() {
    let platform = LinuxPlatform::new_with_startup(
        WindowingModes::all(),
        StartupEnvironment {
            unclaimed_socket: true,
            ..StartupEnvironment::default()
        },
    );
    platform.set_initial_windowing(missing_display());
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        platform.connect_initially()
    }))
    .expect_err("an actual Wayland selection requires explicit socket ownership");
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .expect("the startup refusal must carry its diagnostic");
    assert!(message.contains("WAYLAND_SOCKET requires explicit ownership"));
    assert!(platform.graphical_environment().is_none());
}

#[cfg(feature = "wayland")]
#[test]
fn unused_owned_startup_socket_is_closed_before_headless_launch() {
    use std::{io::Read as _, os::fd::AsRawFd as _, os::unix::net::UnixStream};
    let (socket, mut peer) = UnixStream::pair().unwrap();
    let startup = StartupEnvironment::owned(Some(socket), Some("one-shot-token".into()));
    let socket = startup.wayland_socket.as_ref().unwrap();
    // SAFETY: The startup owner keeps the descriptor alive for this read-only flag query.
    assert_ne!(
        unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let platform = Rc::new(LinuxPlatform::new_with_startup(
        WindowingModes::all(),
        startup,
    ));
    platform.set_initial_windowing(WindowingRequest::Headless);
    platform.run(Box::new({
        let platform = platform.clone();
        move || {
            assert_eq!(peer.read(&mut [0]).unwrap(), 0);
            platform.quit();
        }
    }));
    assert!(platform.startup_environment.borrow().is_none());
}

#[cfg(feature = "wayland")]
#[test]
fn startup_capture_reads_token_once_without_adopting_host_descriptors_or_mutating_environment() {
    const CHILD: &str = "GPUI_LINUX_STARTUP_CAPTURE_CHILD";
    if env::var_os(CHILD).is_some() {
        let startup = StartupEnvironment::capture();
        assert_eq!(
            startup.activation_token.as_deref(),
            Some("startup-capture-token")
        );
        assert!(startup.wayland_socket.is_none());
        assert!(startup.unclaimed_socket);
        assert!(StartupEnvironment::capture().activation_token.is_none());
        assert_eq!(
            env::var("XDG_ACTIVATION_TOKEN").unwrap(),
            "startup-capture-token"
        );
        assert_eq!(env::var("WAYLAND_SOCKET").unwrap(), "76543");
        return;
    }
    let test = format!(
        "{}::startup_capture_reads_token_once_without_adopting_host_descriptors_or_mutating_environment",
        module_path!().split_once("::").unwrap().1
    );
    #[allow(
        clippy::disallowed_methods,
        reason = "startup environment is supplied only to an isolated child process"
    )]
    let output = std::process::Command::new(env::current_exe().unwrap())
        .args(["--exact", &test, "--nocapture"])
        .env(CHILD, "1")
        .env("XDG_ACTIVATION_TOKEN", "startup-capture-token")
        .env("WAYLAND_SOCKET", "76543")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "startup capture child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}
