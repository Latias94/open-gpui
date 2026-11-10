use super::*;
use crate::{NoopTextSystem, TestDispatcher};
use std::{
    cell::Cell,
    sync::atomic::AtomicUsize,
    thread::{self, ThreadId},
};

const DISTANT_TIMER: Duration = Duration::from_secs(60 * 60);

#[test]
fn bench_platform_reuses_dispatcher_without_reusing_platform_or_text_injection() {
    let first_text: Arc<dyn PlatformTextSystem> = Arc::new(NoopTextSystem::new());
    let second_text: Arc<dyn PlatformTextSystem> = Arc::new(NoopTextSystem::new());
    let first = bench_platform(None, first_text.clone());
    let second = bench_platform(None, second_text.clone());

    assert!(!Rc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first.text_system(), &first_text));
    assert!(Arc::ptr_eq(&second.text_system(), &second_text));
    let first_background = first.background_executor();
    let second_background = second.background_executor();
    assert!(Arc::ptr_eq(
        first_background.dispatcher(),
        second_background.dispatcher()
    ));
    for platform in [&first, &second] {
        assert!(Arc::ptr_eq(
            platform.background_executor().dispatcher(),
            platform.foreground_executor().dispatcher()
        ));
        assert!(
            platform
                .background_executor()
                .dispatcher()
                .as_threaded()
                .is_some()
        );
    }
}

#[test]
fn benchmark_constructor_rejects_virtual_and_mismatched_dispatchers() {
    let dispatcher = Arc::new(TestDispatcher::new(0));
    let virtual_background = BackgroundExecutor::new(dispatcher.clone());
    let virtual_foreground = ForegroundExecutor::new(dispatcher);
    let threaded = bench_platform(None, Arc::new(NoopTextSystem::new()));
    let platforms: [Rc<dyn Platform>; 2] = [
        TestPlatform::new(virtual_background, virtual_foreground.clone()),
        TestPlatform::new(threaded.background_executor(), virtual_foreground),
    ];
    criterion().bench_function("reject-invalid-benchmark-dispatcher", |bencher| {
        for platform in &platforms {
            let panic = catch_unwind(AssertUnwindSafe(|| {
                let _cx = BenchAppContext::new(platform.clone(), None, bencher);
            }));
            assert!(panic.is_err());
        }
        bencher.iter(|| ());
    });
}

struct ReleaseChain {
    next: Option<Entity<ReleaseChain>>,
    foreground: ForegroundExecutor,
    released: Rc<Cell<usize>>,
}

impl Drop for ReleaseChain {
    fn drop(&mut self) {
        self.released.set(self.released.get() + 1);
        if let Some(next) = self.next.take() {
            self.foreground.spawn(async move { drop(next) }).detach();
        }
    }
}

#[test]
fn settle_flushes_entity_releases_and_work_queued_by_their_destructors() {
    run_benchmark("settle-release-chain", |mut cx| {
        let released = Rc::new(Cell::new(0));
        let foreground = cx.foreground_executor().clone();
        let leaf = cx.new(|_| ReleaseChain {
            next: None,
            foreground: foreground.clone(),
            released: released.clone(),
        });
        let weak_leaf = leaf.downgrade();
        let middle = cx.new(|_| ReleaseChain {
            next: Some(leaf),
            foreground: foreground.clone(),
            released: released.clone(),
        });
        let root = cx.new(|_| ReleaseChain {
            next: Some(middle),
            foreground,
            released: released.clone(),
        });
        drop(root);
        cx.run_until_idle();
        assert_eq!(released.get(), 0);
        cx.settle();
        assert_eq!(released.get(), 3);
        assert!(weak_leaf.upgrade().is_none());
        cx.bench_iter(|_| ());
        cx.teardown();
    });
}

struct ArmTimerOnDrop {
    background: BackgroundExecutor,
    resumed: Arc<AtomicUsize>,
}

impl Global for ArmTimerOnDrop {}

impl Drop for ArmTimerOnDrop {
    fn drop(&mut self) {
        let timer = self.background.timer(DISTANT_TIMER);
        let resumed = self.resumed.clone();
        self.background
            .spawn(async move {
                timer.await;
                resumed.fetch_add(1, Ordering::SeqCst);
            })
            .detach();
    }
}

struct LocalDrop {
    created_on: ThreadId,
    dropped_on: Rc<Cell<Option<ThreadId>>>,
}

struct QueueTimerOnDrop {
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
    started: Rc<Cell<bool>>,
    resumed: Rc<Cell<bool>>,
}

impl Global for QueueTimerOnDrop {}

impl Drop for QueueTimerOnDrop {
    fn drop(&mut self) {
        let background = self.background.clone();
        let started = self.started.clone();
        let resumed = self.resumed.clone();
        self.foreground
            .spawn(async move {
                started.set(true);
                background.timer(DISTANT_TIMER).await;
                resumed.set(true);
            })
            .detach();
    }
}

#[test]
fn teardown_drains_app_destructor_work_before_checking_timers() {
    run_benchmark("teardown-destructor-queued-timer", |mut cx| {
        let started = Rc::new(Cell::new(false));
        let resumed = Rc::new(Cell::new(false));
        let foreground = cx.foreground_executor().clone();
        let background = cx.background_executor().clone();
        let dispatcher = background.dispatcher().clone();
        cx.update(|app| {
            app.set_global(QueueTimerOnDrop {
                foreground,
                background,
                started: started.clone(),
                resumed: resumed.clone(),
            })
        });
        cx.bench_iter(|_| ());
        assert!(!started.get());
        cx.teardown();
        assert!(
            started.get(),
            "App destructor work must run in its original routine"
        );
        assert!(
            resumed.get(),
            "the newly armed timer must be cancelled and drained"
        );
        let dispatcher = dispatcher.as_threaded().unwrap();
        assert!(dispatcher.is_idle(), "{}", dispatcher.debug_state());
        assert_eq!(dispatcher.cancel_pending_timers(), 0);
    });
}

#[test]
fn teardown_accepts_exactly_one_hundred_cancellation_rounds() {
    let mut checked = false;
    run_benchmark("teardown-exact-timer-cancellation-bound", |mut cx| {
        cx.bench_iter(|_| ());
        if checked {
            cx.teardown();
            return;
        }
        let background = cx.background_executor().clone();
        let dispatcher = background.dispatcher().clone();
        let resumed = Rc::new(Cell::new(0));
        cx.foreground_executor()
            .spawn({
                let resumed = resumed.clone();
                async move {
                    for _ in 0..100 {
                        background.timer(DISTANT_TIMER).await;
                        resumed.set(resumed.get() + 1);
                    }
                }
            })
            .detach();
        cx.run_until_idle();
        cx.teardown();
        assert_eq!(resumed.get(), 100);
        let dispatcher = dispatcher.as_threaded().unwrap();
        assert!(dispatcher.is_idle(), "{}", dispatcher.debug_state());
        assert_eq!(dispatcher.cancel_pending_timers(), 0);
        checked = true;
    });
    assert!(checked);
}

impl Drop for LocalDrop {
    fn drop(&mut self) {
        assert_eq!(thread::current().id(), self.created_on);
        self.dropped_on.set(Some(thread::current().id()));
    }
}

#[test]
fn teardown_releases_app_then_cancels_and_drains_rearmed_timers() {
    run_benchmark("teardown-release-and-timer-rounds", |mut cx| {
        let resumed = Arc::new(AtomicUsize::new(0));
        let foreground_resumptions = Rc::new(Cell::new(0));
        let dropped_on = Rc::new(Cell::new(None));
        let quit_observed = Rc::new(Cell::new(false));
        let background = cx.background_executor().clone();
        let dispatcher = background.dispatcher().clone();
        let weak_app = Rc::downgrade(&cx.app);
        let window = cx.add_empty_window();
        let platform_window = platform_window(&cx, window.window_handle());
        drop(window);
        cx.update(|app| {
            app.set_global(ArmTimerOnDrop {
                background: background.clone(),
                resumed: resumed.clone(),
            });
            app.on_app_quit({
                let quit_observed = quit_observed.clone();
                move |_| {
                    quit_observed.set(true);
                    async {}
                }
            })
            .detach();
        });
        let state = LocalDrop {
            created_on: thread::current().id(),
            dropped_on: dropped_on.clone(),
        };
        cx.foreground_executor()
            .spawn({
                let weak_app = weak_app.clone();
                let foreground_resumptions = foreground_resumptions.clone();
                async move {
                    for _ in 0..2 {
                        background.timer(DISTANT_TIMER).await;
                        assert!(weak_app.upgrade().is_none());
                        foreground_resumptions.set(foreground_resumptions.get() + 1);
                    }
                    drop(state);
                }
            })
            .detach();
        cx.run_until_idle();
        assert_eq!(foreground_resumptions.get(), 0);
        assert_eq!(dropped_on.get(), None);
        cx.bench_iter(|_| ());
        cx.teardown();

        assert!(quit_observed.get());
        assert!(weak_app.upgrade().is_none());
        assert_eq!(foreground_resumptions.get(), 2);
        assert_eq!(resumed.load(Ordering::SeqCst), 1);
        assert_eq!(dropped_on.get(), Some(thread::current().id()));
        assert_eq!(platform_window.presentation_shutdown_counts(), (1, 1, 1));
        let dispatcher = dispatcher
            .as_threaded()
            .expect("threaded benchmark dispatcher");
        assert!(dispatcher.is_idle(), "{}", dispatcher.debug_state());
        assert_eq!(dispatcher.cancel_pending_timers(), 0);
    });
}

#[test]
fn teardown_bounds_timer_rearming_to_one_hundred_cancellation_rounds() {
    let mut checked = false;
    run_benchmark("teardown-timer-cancellation-bound", |mut cx| {
        cx.bench_iter(|_| ());
        if checked {
            cx.teardown();
            return;
        }
        let background = cx.background_executor().clone();
        let dispatcher = background.dispatcher().clone();
        let resumed = Rc::new(Cell::new(0));
        cx.foreground_executor()
            .spawn({
                let resumed = resumed.clone();
                async move {
                    for _ in 0..101 {
                        background.timer(DISTANT_TIMER).await;
                        resumed.set(resumed.get() + 1);
                    }
                }
            })
            .detach();
        cx.run_until_idle();
        let panic = catch_unwind(AssertUnwindSafe(|| cx.teardown()))
            .expect_err("persistent timer rearming must exhaust the cancellation budget");
        let message = panic.downcast_ref::<String>().expect("timer diagnostic");
        assert!(message.contains("benchmark teardown kept scheduling timers"));
        assert!(message.contains("pending_timers: 1"));
        assert_eq!(resumed.get(), 100);
        let dispatcher = dispatcher
            .as_threaded()
            .expect("threaded benchmark dispatcher");
        assert_eq!(dispatcher.cancel_pending_timers(), 1);
        dispatcher.run_until_idle();
        assert_eq!(resumed.get(), 101);
        assert!(dispatcher.is_idle(), "{}", dispatcher.debug_state());
        checked = true;
    });
    assert!(checked);
}

#[test]
fn successive_routines_release_their_app_and_keep_the_shared_dispatcher() {
    criterion().bench_function("successive-benchmark-routines", |bencher| {
        let first_platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
        let first_dispatcher = first_platform.background_executor().dispatcher().clone();
        let mut first = BenchAppContext::new(first_platform, Some("first-routine"), bencher);
        let first_app = Rc::downgrade(&first.app);
        let first_window = first.add_empty_window();
        let first_native = platform_window(&first, first_window.window_handle());
        drop(first_window);
        first.bench_iter(|_| ());
        first.teardown();
        assert!(first_app.upgrade().is_none());
        assert_eq!(first_native.presentation_shutdown_counts(), (1, 1, 1));

        let second_platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
        assert!(Arc::ptr_eq(
            &first_dispatcher,
            second_platform.background_executor().dispatcher()
        ));
        let mut second = BenchAppContext::new(second_platform, Some("second-routine"), bencher);
        assert!(second.read(|app| app.windows.is_empty()));
        let second_app = Rc::downgrade(&second.app);
        let second_window = second.add_empty_window();
        assert!(
            second_window
                .window_handle()
                .update(&mut second, |_, window, _| { window.is_window_active() })
                .expect("the second routine window must be active")
        );
        drop(second_window);
        second.bench_iter(|_| ());
        second.teardown();
        assert!(second_app.upgrade().is_none());
        assert!(first_app.upgrade().is_none());
    });
}

#[test]
fn completed_terminal_shutdown_remains_idempotent_during_teardown() {
    run_benchmark("benchmark-repeated-terminal-shutdown", |mut cx| {
        let quits = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_app_quit({
                let quits = quits.clone();
                move |_| {
                    quits.set(quits.get() + 1);
                    async {}
                }
            })
            .detach();
        });
        cx.bench_iter(|_| ());
        cx.update(|app| app.quit_after_terminal_shutdown());
        cx.run_until_idle();
        assert!(cx.read(|app| app.quitting));
        cx.update(|app| {
            app.shutdown();
            app.shutdown_for_native_exit_test();
            app.quit_after_terminal_shutdown();
        });
        cx.teardown();
        assert_eq!(quits.get(), 1);
    });
}

#[test]
fn terminal_shutdown_accepts_a_late_platform_quit_without_another_fence() {
    criterion().bench_function("benchmark-late-terminal-platform-quit", |bencher| {
        let base = bench_platform(None, Arc::new(NoopTextSystem::new()));
        let platform = TestPlatform::new(base.background_executor(), base.foreground_executor());
        let mut cx = BenchAppContext::new(platform.clone(), None, bencher);
        let quits = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_app_quit({
                let quits = quits.clone();
                move |_| {
                    quits.set(quits.get() + 1);
                    async {}
                }
            })
            .detach();
        });
        cx.bench_iter(|_| ());
        cx.update(|app| app.shutdown_for_native_exit_test());
        cx.run_until_idle();
        assert!(cx.read(|app| app.quitting));
        assert!(!platform.did_quit());
        cx.update(|app| app.quit_after_terminal_shutdown());
        assert!(platform.did_quit());
        cx.teardown();
        assert_eq!(quits.get(), 1);
    });
}

#[test]
fn late_platform_quit_survives_update_unwind_without_another_app_borrow() {
    criterion().bench_function("benchmark-late-terminal-quit-unwind", |bencher| {
        let base = bench_platform(None, Arc::new(NoopTextSystem::new()));
        let platform = TestPlatform::new(base.background_executor(), base.foreground_executor());
        let mut cx = BenchAppContext::new(platform.clone(), None, bencher);
        let dispatcher = cx.background_executor().dispatcher().clone();
        cx.bench_iter(|_| ());
        cx.update(|app| app.shutdown_for_native_exit_test());
        cx.run_until_idle();
        assert!(!platform.did_quit());
        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                app.quit_after_terminal_shutdown();
                panic!("injected late terminal quit failure");
            })
        }));
        assert!(panic.is_err());
        dispatcher.as_threaded().unwrap().run_until_idle();
        assert!(
            platform.did_quit(),
            "the late quit must not require another App borrow"
        );
        cx.teardown();
    });
}

#[test]
fn benchmark_context_blocks_http_without_a_configurable_fake_client() {
    run_benchmark("benchmark-blocked-http", |mut cx| {
        let client = cx.read(|app| app.http_client());
        let request = open_gpui_http_client::Request::builder()
            .uri("https://benchmark.invalid/blocked")
            .body(open_gpui_http_client::AsyncBody::default())
            .expect("the request must be valid");
        let Err(error) = futures::executor::block_on(client.send(request)) else {
            panic!("benchmark HTTP must be blocked");
        };
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .expect("blocked HTTP error")
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        cx.bench_iter(|_| ());
        cx.teardown();
    });
}
