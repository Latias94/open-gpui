//! Scoped activity controls from Zed 450775244fade36db70af2447be24e74edb43368,
//! reconciled with fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! These tests exercise Core and TestPlatform without qualifying native power policy.

use super::Application;
use crate::{App, Platform, TestAppContext, TestPlatform, VisualTestPlatform};
use futures::{FutureExt as _, channel::oneshot};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    time::Duration,
};

#[crate::test]
fn immediate_acquisition_is_owned_by_the_unpolled_ready_task(cx: &mut TestAppContext) {
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    let acquisition = cx.read(|app| app.prevent_idle_sleep("unpolled acquisition"));
    assert!(acquisition.is_ready());
    assert_eq!(cx.active_idle_sleep_preventions(), 1);

    drop(acquisition);

    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn nested_guards_release_only_their_own_acquisition(cx: &mut TestAppContext) {
    let outer = cx
        .read(|app| app.prevent_idle_sleep("outer operation"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("outer acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    {
        let inner = cx
            .read(|app| app.prevent_idle_sleep("inner operation"))
            .now_or_never()
            .expect("immediate acquisition must be ready")
            .expect("inner acquisition must succeed");
        assert_eq!(cx.active_idle_sleep_preventions(), 2);
        {
            let _nested = cx
                .read(|app| app.prevent_idle_sleep("nested operation"))
                .now_or_never()
                .expect("immediate acquisition must be ready")
                .expect("nested acquisition must succeed");
            assert_eq!(cx.active_idle_sleep_preventions(), 3);
        }
        assert_eq!(cx.active_idle_sleep_preventions(), 2);
        drop(outer);
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
        drop(inner);
    }
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn delayed_acquisition_waits_for_the_existing_executor_timer(cx: &mut TestAppContext) {
    cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
    let acquisition = cx.read(|app| app.prevent_idle_sleep("delayed operation"));
    assert!(!acquisition.is_ready());
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!acquisition.is_ready());
    assert_eq!(cx.active_idle_sleep_preventions(), 0);

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(acquisition.is_ready());
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    let guard = acquisition
        .now_or_never()
        .expect("the acquisition deadline must have elapsed")
        .expect("delayed acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    drop(guard);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn delayed_cancellation_before_poll_or_during_acquisition_cannot_acquire_later(
    cx: &mut TestAppContext,
) {
    for fails in [false, true] {
        for poll_before_cancel in [false, true] {
            cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
            cx.set_idle_sleep_prevention_fails(fails);
            let acquisition = cx.read(|app| app.prevent_idle_sleep("cancel pending acquisition"));
            if poll_before_cancel {
                cx.run_until_parked();
                cx.executor().advance_clock(Duration::from_secs(1));
                cx.run_until_parked();
            }
            assert_eq!(cx.active_idle_sleep_preventions(), 0);
            assert!(!acquisition.is_ready());

            drop(acquisition);
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_secs(4));
            cx.run_until_parked();

            assert_eq!(cx.active_idle_sleep_preventions(), 0);
        }
    }
}

#[crate::test]
fn dropping_completed_delayed_task_releases_its_unclaimed_guard(cx: &mut TestAppContext) {
    cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
    let acquisition = cx.read(|app| app.prevent_idle_sleep("unclaimed delayed result"));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(acquisition.is_ready());
    assert_eq!(cx.active_idle_sleep_preventions(), 1);

    drop(acquisition);
    cx.run_until_parked();

    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    cx.executor().advance_clock(Duration::from_secs(4));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn cancelling_an_owning_task_releases_an_acquired_guard(cx: &mut TestAppContext) {
    for delay in [Duration::ZERO, Duration::from_secs(2)] {
        cx.set_idle_sleep_prevention_delay(delay);
        let acquisition = cx.read(|app| app.prevent_idle_sleep("cancel active operation"));
        let (finish, finished) = oneshot::channel::<()>();
        let operation = cx.foreground_executor.spawn(async move {
            let _guard = acquisition.await.expect("acquisition must succeed");
            finished.await.expect("the operation should remain pending");
        });
        cx.run_until_parked();
        cx.executor().advance_clock(delay);
        cx.run_until_parked();
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
        assert!(!operation.is_ready());
        assert!(!finish.is_canceled());

        drop(operation);
        cx.run_until_parked();

        assert!(finish.is_canceled());
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
    }
}

#[crate::test]
fn acquisition_errors_preserve_existing_guards_and_allow_a_new_request(cx: &mut TestAppContext) {
    let retained = cx
        .read(|app| app.prevent_idle_sleep("retained operation"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("retained acquisition must succeed");
    for delay in [Duration::ZERO, Duration::from_secs(2)] {
        cx.set_idle_sleep_prevention_delay(delay);
        cx.set_idle_sleep_prevention_fails(true);
        let reason = String::from("failing operation 🦀");
        let acquisition = cx.read(|app| app.prevent_idle_sleep(&reason));
        drop(reason);
        cx.run_until_parked();
        cx.executor().advance_clock(delay);
        cx.run_until_parked();
        let error = acquisition
            .now_or_never()
            .expect("the failed acquisition must be ready")
            .err()
            .expect("the configured acquisition must fail");
        assert_eq!(
            error.to_string(),
            "Idle sleep prevention for \"failing operation 🦀\" is set to fail in this test"
        );
        assert_eq!(cx.active_idle_sleep_preventions(), 1);

        cx.set_idle_sleep_prevention_fails(false);
        let retry = cx.read(|app| app.prevent_idle_sleep("retry operation"));
        cx.run_until_parked();
        cx.executor().advance_clock(delay);
        cx.run_until_parked();
        let retry = retry
            .now_or_never()
            .expect("the retry deadline must have elapsed")
            .expect("a new request must succeed after clearing the failure");
        assert_eq!(cx.active_idle_sleep_preventions(), 2);
        drop(retry);
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
    }
    drop(retained);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn delay_failure_and_reason_are_snapshotted_for_each_request(cx: &mut TestAppContext) {
    cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
    let success = cx.read(|app| app.prevent_idle_sleep("already requested success"));
    cx.set_idle_sleep_prevention_delay(Duration::from_secs(1));
    cx.set_idle_sleep_prevention_fails(true);
    let reason = String::from("owned failure reason");
    let failure = cx.read(|app| app.prevent_idle_sleep(&reason));
    drop(reason);
    cx.set_idle_sleep_prevention_delay(Duration::ZERO);
    cx.set_idle_sleep_prevention_fails(false);

    cx.run_until_parked();
    assert!(!success.is_ready());
    assert!(!failure.is_ready());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!success.is_ready());
    let error = failure
        .now_or_never()
        .expect("the original failure deadline must be retained")
        .err()
        .expect("clearing the failure must not change an existing request");
    assert!(error.to_string().contains("owned failure reason"));
    assert_eq!(cx.active_idle_sleep_preventions(), 0);

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let guard = success
        .now_or_never()
        .expect("the original success deadline must be retained")
        .expect("later failure settings must not change an existing request");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    drop(guard);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn requests_and_guards_are_independent_between_apps(cx: &mut TestAppContext) {
    let other = cx.new_app();
    other.set_idle_sleep_prevention_delay(Duration::from_secs(2));
    let first = cx
        .read(|app| app.prevent_idle_sleep("first app"))
        .now_or_never()
        .expect("the first app must use its own immediate acquisition")
        .expect("the first acquisition must succeed");
    let second = other.read(|app| app.prevent_idle_sleep("second app"));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    assert_eq!(other.active_idle_sleep_preventions(), 0);
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    let second = second
        .now_or_never()
        .expect("the shared scheduler must complete the second deadline")
        .expect("the second acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    assert_eq!(other.active_idle_sleep_preventions(), 1);

    drop(first);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    assert_eq!(other.active_idle_sleep_preventions(), 1);
    other.quit();
    cx.run_until_parked();
    assert_eq!(other.active_idle_sleep_preventions(), 1);
    drop(second);
    assert_eq!(other.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn retained_guard_does_not_retain_the_app_or_end_at_app_shutdown(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    let mut application = Application::with_platform(platform.clone());
    let weak_app = Rc::downgrade(&application.0);
    let guard = application
        .update_for_test(|app| app.prevent_idle_sleep("outlive application"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("acquisition must succeed");
    application.update_for_test(App::shutdown);
    drop(application);
    cx.run_until_parked();

    assert!(weak_app.upgrade().is_none());
    assert_eq!(platform.active_idle_sleep_preventions(), 1);
    drop(guard);
    assert_eq!(platform.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn pending_acquisition_outlives_both_the_app_and_platform(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    platform.set_idle_sleep_prevention_delay(Duration::from_secs(2));
    let weak_platform = Rc::downgrade(&platform);
    let mut application = Application::with_platform(platform.clone());
    let weak_app = Rc::downgrade(&application.0);
    let acquisition = application
        .update_for_test(|app| app.prevent_idle_sleep("outlive application and platform"));
    cx.run_until_parked();
    assert!(!acquisition.is_ready());
    assert_eq!(platform.active_idle_sleep_preventions(), 0);
    application.update_for_test(App::shutdown);
    drop(application);
    drop(platform);
    cx.run_until_parked();
    assert!(weak_app.upgrade().is_none());
    assert!(weak_platform.upgrade().is_none());

    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    let guard = acquisition
        .now_or_never()
        .expect("the independent acquisition must complete")
        .expect("dropping the app and platform must not invalidate the acquisition");
    assert!(weak_app.upgrade().is_none());
    assert!(weak_platform.upgrade().is_none());
    drop(guard);
}

#[cfg(not(target_family = "wasm"))]
#[crate::test]
fn acquired_guard_can_be_released_on_a_worker_thread(cx: &mut TestAppContext) {
    let guard = cx
        .read(|app| app.prevent_idle_sleep("worker release"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    let main_thread = std::thread::current().id();

    std::thread::spawn(move || {
        assert_ne!(std::thread::current().id(), main_thread);
        drop(guard);
    })
    .join()
    .expect("the worker must release the guard");

    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn unwinding_releases_one_guard_and_preserves_other_activity(cx: &mut TestAppContext) {
    let survivor = cx
        .read(|app| app.prevent_idle_sleep("surviving operation"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("surviving acquisition must succeed");
    let guard = cx
        .read(|app| app.prevent_idle_sleep("unwinding operation"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("unwinding acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 2);

    let result = catch_unwind(AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("exercise activity cleanup during unwinding");
    }));

    assert!(result.is_err());
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    drop(survivor);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn visual_test_platform_explicitly_rejects_idle_sleep_prevention(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    let visual = VisualTestPlatform::new(platform.clone(), 0);
    let error = visual
        .prevent_idle_sleep("visual operation")
        .now_or_never()
        .expect("unsupported acquisition must be ready")
        .err()
        .expect("visual tests must report unsupported acquisition");

    assert_eq!(
        error.to_string(),
        "Idle sleep prevention for \"visual operation\" is not supported in visual tests"
    );
    visual.dispatcher().run_until_parked();
    cx.run_until_parked();
    assert_eq!(platform.active_idle_sleep_preventions(), 0);
}

#[crate::test]
fn app_nap_noops_do_not_acquire_or_release_system_sleep_guards(cx: &mut TestAppContext) {
    let background = cx.executor();
    let first_nap = background.prevent_app_nap("background operation");
    let second_nap = background
        .dispatcher()
        .prevent_app_nap("dispatcher operation");
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    let idle_sleep = cx
        .read(|app| app.prevent_idle_sleep("system sleep operation"))
        .now_or_never()
        .expect("immediate acquisition must be ready")
        .expect("system sleep acquisition must succeed");
    assert_eq!(cx.active_idle_sleep_preventions(), 1);

    drop(first_nap);
    drop(second_nap);
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    drop(idle_sleep);
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}
