use super::{BackgroundExecutor, ForegroundExecutor};
use crate::{TestDispatcher, ThreadedDispatcher};
use futures::FutureExt;
use std::{sync::Arc, thread};

#[test]
fn deterministic_and_threaded_executors_keep_distinct_runtime_identity() {
    let threaded_dispatcher = Arc::new(ThreadedDispatcher::new());
    let threaded_background = BackgroundExecutor::new(threaded_dispatcher.clone());
    let threaded_foreground = ForegroundExecutor::new(threaded_dispatcher.clone());
    assert!(!threaded_background.is_test());
    assert!(!threaded_background.scheduler_executor().is_test());
    assert!(!threaded_foreground.scheduler_executor().is_test());

    let test_dispatcher = Arc::new(TestDispatcher::new(17));
    let test_background = BackgroundExecutor::new(test_dispatcher.clone());
    let test_foreground = ForegroundExecutor::new(test_dispatcher.clone());
    assert!(open_gpui_scheduler::test_scheduler_created());

    let threaded_inner = threaded_background.scheduler_executor();
    let test_inner = test_background.scheduler_executor();
    let threaded_owner_count = Arc::strong_count(threaded_inner.scheduler());
    let test_owner_count = Arc::strong_count(test_inner.scheduler());
    assert!(!threaded_background.clone().is_test());
    assert!(!threaded_inner.is_test());
    assert!(!threaded_foreground.scheduler_executor().is_test());
    assert!(test_background.clone().is_test());
    assert!(test_inner.is_test());
    assert!(test_foreground.scheduler_executor().is_test());
    assert_eq!(
        Arc::strong_count(threaded_inner.scheduler()),
        threaded_owner_count
    );
    assert_eq!(Arc::strong_count(test_inner.scheduler()), test_owner_count);

    let weak_threaded_scheduler = Arc::downgrade(threaded_inner.scheduler());
    let weak_threaded_local_scheduler = Arc::downgrade(threaded_foreground.inner.scheduler());
    let weak_test_scheduler = Arc::downgrade(test_dispatcher.scheduler());
    let weak_threaded_dispatcher = Arc::downgrade(&threaded_dispatcher);
    let weak_test_dispatcher = Arc::downgrade(&test_dispatcher);

    let threaded_background_task = threaded_background.spawn({
        let executor = threaded_background.clone();
        async move { (executor.is_test(), thread::current().id()) }
    });
    let test_background_task = test_background.spawn({
        let executor = test_background.clone();
        async move { (executor.is_test(), thread::current().id()) }
    });
    let threaded_foreground_task = threaded_foreground.spawn({
        let executor = threaded_foreground.scheduler_executor();
        async move { (executor.is_test(), thread::current().id()) }
    });
    let test_foreground_task = test_foreground.spawn({
        let executor = test_foreground.scheduler_executor();
        async move { (executor.is_test(), thread::current().id()) }
    });

    test_dispatcher.run_until_parked();
    threaded_dispatcher.run_until_idle();

    let main_thread = thread::current().id();
    let (is_test, worker_thread) = threaded_background_task
        .now_or_never()
        .expect("threaded background work should finish on a worker thread");
    assert!(!is_test);
    assert_ne!(worker_thread, main_thread);
    assert_eq!(
        threaded_foreground_task.now_or_never(),
        Some((false, main_thread))
    );
    assert_eq!(
        test_background_task.now_or_never(),
        Some((true, main_thread))
    );
    assert_eq!(
        test_foreground_task.now_or_never(),
        Some((true, main_thread))
    );

    drop((
        threaded_background,
        threaded_foreground,
        threaded_inner,
        threaded_dispatcher,
        test_background,
        test_foreground,
        test_inner,
        test_dispatcher,
    ));
    assert!(weak_threaded_scheduler.upgrade().is_none());
    assert!(weak_threaded_local_scheduler.upgrade().is_none());
    assert!(weak_test_scheduler.upgrade().is_none());
    assert!(weak_threaded_dispatcher.upgrade().is_none());
    assert!(weak_test_dispatcher.upgrade().is_none());
    assert!(open_gpui_scheduler::test_scheduler_created());
}
