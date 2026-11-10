//! Public scoped activity consumers of Zed 450775244fade36db70af2447be24e74edb43368,
//! retained at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! Native-free guard and TestAppContext coverage does not qualify native sleep inhibition.

use anyhow::Result;
use open_gpui::{ActivityGuard, App, BackgroundExecutor, Platform, PlatformDispatcher, Task};
use std::sync::{Arc, Mutex};

#[test]
fn scoped_activity_api_is_public_without_test_support() {
    fn assert_send<T: Send + 'static>() {}

    assert_send::<ActivityGuard>();
    assert_send::<Task<Result<ActivityGuard>>>();
    let _: fn(&App, &str) -> Task<Result<ActivityGuard>> = App::prevent_idle_sleep;
    let _: fn(&dyn Platform, &str) -> Task<Result<ActivityGuard>> =
        <dyn Platform>::prevent_idle_sleep;
    let _: fn(&BackgroundExecutor, &str) -> ActivityGuard = BackgroundExecutor::prevent_app_nap;
    let _: fn(&(dyn PlatformDispatcher + 'static), &str) -> ActivityGuard =
        <dyn PlatformDispatcher>::prevent_app_nap;
    let _: fn() -> ActivityGuard = ActivityGuard::noop;
    drop(ActivityGuard::noop());
}

#[test]
fn moved_guard_runs_its_owning_once_callback_only_on_drop() {
    let releases = Arc::new(Mutex::new(Vec::new()));
    let reason = String::from("owned release payload");
    let guard = ActivityGuard::new({
        let releases = releases.clone();
        move || {
            releases
                .lock()
                .expect("release log must be available")
                .push(reason);
        }
    });
    let owner = Some(Box::new(guard));
    assert!(releases.lock().unwrap().is_empty());

    drop(owner);

    assert_eq!(*releases.lock().unwrap(), ["owned release payload"]);
}

#[test]
fn public_guard_releases_once_during_unwinding() {
    let releases = Arc::new(Mutex::new(Vec::new()));
    let result = std::panic::catch_unwind({
        let releases = releases.clone();
        move || {
            let _guard = ActivityGuard::new(move || {
                releases
                    .lock()
                    .expect("release log must be available")
                    .push("unwind");
            });
            panic!("exercise public guard unwinding");
        }
    });

    assert!(result.is_err());
    assert_eq!(*releases.lock().unwrap(), ["unwind"]);
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn public_guard_runs_release_on_the_dropping_worker() {
    let releases = Arc::new(Mutex::new(Vec::new()));
    let guard = ActivityGuard::new({
        let releases = releases.clone();
        move || {
            releases
                .lock()
                .expect("release log must be available")
                .push(std::thread::current().id());
        }
    });
    let main_thread = std::thread::current().id();

    let worker_thread = std::thread::spawn(move || {
        drop(guard);
        std::thread::current().id()
    })
    .join()
    .expect("the worker must release its activity");

    assert_ne!(worker_thread, main_thread);
    assert_eq!(*releases.lock().unwrap(), [worker_thread]);
}

#[cfg(feature = "test-support")]
mod test_contexts {
    use super::*;
    use futures::{FutureExt as _, channel::oneshot};
    use open_gpui::{AppContext as _, Priority, RunnableVariant, TestAppContext, TestDispatcher};
    use std::{rc::Rc, time::Duration};

    #[open_gpui::test]
    fn public_background_work_holds_activity_until_completion(cx: &mut TestAppContext) {
        cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
        let acquisition = cx.read(|app| app.prevent_idle_sleep("background consumer"));
        let (finish, finished) = oneshot::channel::<()>();
        let operation = cx.executor().spawn(async move {
            let _guard = acquisition.await.expect("the activity must be acquired");
            finished.await
        });
        cx.run_until_parked();
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
        assert!(!operation.is_ready());

        finish
            .send(())
            .expect("the operation must still be waiting");
        cx.run_until_parked();
        operation
            .now_or_never()
            .expect("the operation must finish")
            .expect("the operation must receive completion");
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
    }

    #[open_gpui::test]
    fn public_task_cancellation_releases_pending_or_acquired_activity(cx: &mut TestAppContext) {
        for cancel_after_acquisition in [false, true] {
            cx.set_idle_sleep_prevention_delay(Duration::from_secs(2));
            let acquisition = cx.read(|app| app.prevent_idle_sleep("cancelled consumer"));
            let (finish, finished) = oneshot::channel::<()>();
            let operation = cx.foreground_executor().spawn(async move {
                let _guard = acquisition.await.expect("the activity must be acquired");
                finished.await.expect("the operation must remain pending");
            });
            cx.run_until_parked();
            assert_eq!(cx.active_idle_sleep_preventions(), 0);
            let elapsed = if cancel_after_acquisition { 2 } else { 1 };
            cx.executor().advance_clock(Duration::from_secs(elapsed));
            cx.run_until_parked();
            assert_eq!(
                cx.active_idle_sleep_preventions(),
                usize::from(cancel_after_acquisition)
            );

            drop(operation);
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_secs(4));
            cx.run_until_parked();

            assert!(finish.is_canceled());
            assert_eq!(cx.active_idle_sleep_preventions(), 0);
        }
    }

    #[open_gpui::test]
    fn public_failure_controls_preserve_the_owned_reason_and_allow_recovery(
        cx: &mut TestAppContext,
    ) {
        cx.set_idle_sleep_prevention_delay(Duration::from_secs(1));
        cx.set_idle_sleep_prevention_fails(true);
        let reason = String::from("temporary consumer reason 🦀");
        let acquisition = cx.read(|app| app.prevent_idle_sleep(&reason));
        drop(reason);
        cx.set_idle_sleep_prevention_fails(false);
        cx.set_idle_sleep_prevention_delay(Duration::ZERO);
        cx.run_until_parked();
        assert!(!acquisition.is_ready());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        let error = acquisition
            .now_or_never()
            .expect("the failed acquisition deadline must have elapsed")
            .err()
            .expect("the original failure setting must be retained");
        assert_eq!(
            error.to_string(),
            "Idle sleep prevention for \"temporary consumer reason 🦀\" is set to fail in this test"
        );
        assert_eq!(cx.active_idle_sleep_preventions(), 0);

        let guard = cx
            .read(|app| app.prevent_idle_sleep("recovered consumer"))
            .now_or_never()
            .expect("the new delay setting must apply to new requests")
            .expect("a new request must succeed after clearing the failure");
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
        drop(guard);
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
    }

    #[open_gpui::test]
    fn public_entity_and_application_lifetimes_own_independent_guards(cx: &mut TestAppContext) {
        let other = cx.new_app();
        let guard = cx
            .read(|app| app.prevent_idle_sleep("entity consumer"))
            .now_or_never()
            .expect("the entity acquisition must be ready")
            .expect("the entity acquisition must succeed");
        let entity = cx.new(move |_| guard);
        let weak_entity = entity.downgrade();
        let other_guard = other
            .read(|app| app.prevent_idle_sleep("independent app consumer"))
            .now_or_never()
            .expect("the other app acquisition must be ready")
            .expect("the other app acquisition must succeed");
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
        assert_eq!(other.active_idle_sleep_preventions(), 1);

        cx.update(|_| drop(entity));
        cx.run_until_parked();
        assert!(weak_entity.upgrade().is_none());
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
        assert_eq!(other.active_idle_sleep_preventions(), 1);
        other.quit();
        cx.run_until_parked();
        assert_eq!(other.active_idle_sleep_preventions(), 1);
        let weak_app = Rc::downgrade(&other.app);
        drop(other);
        cx.run_until_parked();
        assert!(weak_app.upgrade().is_none());
        drop(other_guard);
    }

    struct RecordingNapDispatcher {
        inner: TestDispatcher,
        events: Arc<Mutex<Vec<String>>>,
    }

    impl PlatformDispatcher for RecordingNapDispatcher {
        fn is_main_thread(&self) -> bool {
            self.inner.is_main_thread()
        }

        fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
            self.inner.dispatch(runnable, priority);
        }

        fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
            self.inner.dispatch_on_main_thread(runnable, priority)
        }

        fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
            self.inner.dispatch_after(duration, runnable);
        }

        fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
            self.inner.spawn_realtime(f);
        }

        fn as_test(&self) -> Option<&TestDispatcher> {
            Some(&self.inner)
        }

        fn prevent_app_nap(&self, reason: &str) -> ActivityGuard {
            let reason = reason.to_owned();
            self.events.lock().unwrap().push(format!("begin:{reason}"));
            let events = self.events.clone();
            ActivityGuard::new(move || {
                events.lock().unwrap().push(format!("end:{reason}"));
            })
        }
    }

    #[open_gpui::test]
    fn public_executor_forwards_app_nap_to_its_dispatcher(cx: &mut TestAppContext) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(RecordingNapDispatcher {
            inner: cx.dispatcher.clone(),
            events: events.clone(),
        });
        let executor = BackgroundExecutor::new(dispatcher.clone());
        let reason = String::from("executor reason 🦀");
        let guard = executor.prevent_app_nap(&reason);
        drop(reason);
        let direct = dispatcher.prevent_app_nap("direct reason");
        assert_eq!(
            *events.lock().unwrap(),
            ["begin:executor reason 🦀", "begin:direct reason"]
        );
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
        drop(executor);
        drop(dispatcher);

        drop(guard);
        drop(direct);

        assert_eq!(
            *events.lock().unwrap(),
            [
                "begin:executor reason 🦀",
                "begin:direct reason",
                "end:executor reason 🦀",
                "end:direct reason",
            ]
        );
        assert_eq!(cx.active_idle_sleep_preventions(), 0);
    }
}
