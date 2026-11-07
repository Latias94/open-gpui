use super::super::NativeRetirementRetryWake;
use super::*;
use windows::Win32::UI::WindowsAndMessaging::WM_TIMER;

fn retirement_timer_id(platform: &WindowsPlatform) -> usize {
    match platform.inner.native_retirement.borrow().retry.wake {
        Some(NativeRetirementRetryWake::Timer(id)) => id,
        other => panic!("expected an admitted native retirement timer, got {other:?}"),
    }
}

#[test]
fn native_retirement_timer_admission_failure_posts_cleanup_without_retaining_a_task() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("retirement timer fallback backend"));
    let hwnd = platform.handle;
    let lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let capture = Arc::new(());
    let callback_lifetime = Arc::downgrade(&capture);
    platform.on_system_sleep(Arc::new(move || {
        let _keep_alive = &capture;
    }));
    lifecycle.fail_next_platform_destroy();
    lifecycle.fail_next_retirement_timer.set(true);
    drop(platform);
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.retirement_timer_attempts.get(), 1);
    assert!(callback_lifetime.upgrade().is_some());
    assert_eq!(
        lifetime
            .upgrade()
            .unwrap()
            .native_retirement
            .borrow()
            .retry
            .wake,
        Some(NativeRetirementRetryWake::MainThreadMessage),
    );
    pump_messages_until_with_delay(
        "posted retirement fallback releases exact platform and callback ownership",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && lifecycle.ole_uninitialize_count() == 1
                && lifetime.upgrade().is_none()
                && callback_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle.platform_destroy_attempts(), 2);
    assert_eq!(lifecycle.retirement_timer_attempts.get(), 1);
}

#[test]
fn native_retirement_timer_cancels_before_foreground_maintenance_destroys_its_transport() {
    discard_stale_quit_messages();
    let platform =
        Rc::new(WindowsPlatform::new(true).expect("early retirement completion backend"));
    let hwnd = platform.handle;
    let lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let capture = Arc::new(());
    let callback_lifetime = Arc::downgrade(&capture);
    platform.on_system_sleep(Arc::new(move || {
        let _keep_alive = &capture;
    }));
    lifecycle.fail_next_platform_destroy();
    platform.begin_native_finalization();
    let _scheduled = retirement_timer_id(&platform);
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(callback_lifetime.upgrade().is_some());
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(platform.inner.run_foreground_task(), Some(0));
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    assert_eq!(lifecycle.platform_destroy_attempts(), 2);
    assert_eq!(lifecycle.retirement_timer_attempts.get(), 1);
    assert!(platform.inner.native_retirement_is_settled());
    drop(platform);
    assert!(lifetime.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
}

#[test]
fn native_retirement_timer_rejects_cancelled_and_previous_platform_tokens() {
    discard_stale_quit_messages();
    let mut previous_platform_timer = None;
    for _ in 0..2 {
        let platform =
            Rc::new(WindowsPlatform::new(true).expect("retirement timer identity backend"));
        let hwnd = platform.handle;
        let lifetime = Rc::downgrade(&platform.inner);
        let lifecycle = platform.lifecycle_test_probe.clone();
        lifecycle.fail_next_platform_destroy();
        platform.begin_native_finalization();
        let cancelled = retirement_timer_id(&platform);
        platform.inner.cancel_native_retirement_retry();
        platform.inner.schedule_native_retirement_retry();
        let current = retirement_timer_id(&platform);
        assert_ne!(cancelled, current);
        platform.inner.schedule_native_retirement_retry();
        assert_eq!(retirement_timer_id(&platform), current);
        assert_eq!(lifecycle.retirement_timer_attempts.get(), 2);
        for stale in [Some(cancelled), previous_platform_timer]
            .into_iter()
            .flatten()
        {
            assert_ne!(stale, current);
            unsafe { SendMessageW(hwnd, WM_TIMER, Some(WPARAM(stale)), None) };
            assert_eq!(lifecycle.platform_destroy_attempts(), 1);
            assert_eq!(retirement_timer_id(&platform), current);
        }
        previous_platform_timer = Some(current);
        drop(platform);
        pump_messages_until_with_delay(
            "current native retirement timer releases its exact platform",
            Duration::from_millis(2),
            || {
                !unsafe { IsWindow(Some(hwnd)).as_bool() }
                    && lifecycle.ole_uninitialize_count() == 1
                    && lifetime.upgrade().is_none()
            },
        );
        assert_eq!(lifecycle.platform_destroy_attempts(), 2);
        assert_eq!(lifecycle.retirement_timer_attempts.get(), 2);
    }
}
