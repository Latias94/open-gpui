#![cfg(not(target_family = "wasm"))]

use futures::FutureExt;
use open_gpui_scheduler::{
    BackgroundExecutor, Scheduler, TestClock, TestScheduler, TestSchedulerConfig,
    test_scheduler_created,
};
use std::{sync::Arc, thread};

#[test]
fn process_hint_and_executor_identity_follow_scheduler_ownership() {
    assert!(!test_scheduler_created());
    let config = TestSchedulerConfig::with_seed(17);
    let _clock = TestClock::new();
    assert!(!test_scheduler_created());

    let scheduler = Arc::new(TestScheduler::new(config));
    let weak_scheduler = Arc::downgrade(&scheduler);
    assert!(test_scheduler_created());

    let local = scheduler.foreground();
    let background = scheduler.background();
    let erased_scheduler: Arc<dyn Scheduler> = scheduler.clone();
    assert!(Arc::ptr_eq(local.scheduler(), &erased_scheduler));
    assert!(Arc::ptr_eq(background.scheduler(), &erased_scheduler));
    drop(erased_scheduler);

    let owner_count = Arc::strong_count(&scheduler);
    assert!(local.is_test());
    assert!(background.is_test());
    assert_eq!(Arc::strong_count(&scheduler), owner_count);

    let main_thread = thread::current().id();
    let local_task = local.spawn({
        let executor = local.clone();
        async move { (executor.is_test(), thread::current().id()) }
    });
    let background_task = background.spawn({
        let executor = background.clone();
        async move { (executor.is_test(), thread::current().id()) }
    });
    let dedicated_task = background.spawn_dedicated(|executor| async move {
        let background = BackgroundExecutor::new(executor.scheduler().clone());
        (
            executor.is_test(),
            background.is_test(),
            thread::current().id(),
        )
    });

    scheduler.run();
    assert_eq!(local_task.now_or_never(), Some((true, main_thread)));
    assert_eq!(background_task.now_or_never(), Some((true, main_thread)));
    assert_eq!(
        dedicated_task.now_or_never(),
        Some((true, true, main_thread))
    );

    let local_clone = local.clone();
    let background_clone = background.clone();
    drop((local, background, scheduler));
    assert!(weak_scheduler.upgrade().is_some());
    assert!(local_clone.is_test());
    assert!(background_clone.is_test());

    drop(local_clone);
    assert!(weak_scheduler.upgrade().is_some());
    thread::spawn(move || {
        assert!(background_clone.is_test());
        assert!(test_scheduler_created());
    })
    .join()
    .expect("runtime detection should also work on another thread");

    assert!(weak_scheduler.upgrade().is_none());
    assert!(test_scheduler_created());
}
