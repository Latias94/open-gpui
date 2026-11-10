use crate::{PlatformDispatcher, RunnableMeta};
use async_task::Runnable;
use chrono::{DateTime, Utc};
use futures::channel::oneshot;
use open_gpui_scheduler::Instant;
use open_gpui_scheduler::{
    Clock, LocalExecutor, Priority, Scheduler, SessionId, Task, TestScheduler, Timer,
    spawn_dedicated_thread,
};
#[cfg(not(target_family = "wasm"))]
use std::task::{Context, Poll};
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    time::Duration,
};

/// A production implementation of [`Scheduler`] that wraps a [`PlatformDispatcher`].
///
/// This allows GPUI to use the scheduler crate's executor types with the platform's
/// native dispatch mechanisms (e.g., Grand Central Dispatch on macOS).
pub struct PlatformScheduler {
    dispatcher: Arc<dyn PlatformDispatcher>,
    clock: Arc<PlatformClock>,
    next_session_id: AtomicU16,
    #[cfg(feature = "profiler")]
    foreground_runnables: crate::profiler::journal::ForegroundRunnableCounter,
}

impl PlatformScheduler {
    pub fn new(dispatcher: Arc<dyn PlatformDispatcher>) -> Self {
        Self {
            dispatcher: dispatcher.clone(),
            clock: Arc::new(PlatformClock { dispatcher }),
            next_session_id: AtomicU16::new(0),
            #[cfg(feature = "profiler")]
            foreground_runnables: crate::profiler::journal::foreground_runnable_counter(),
        }
    }

    pub fn foreground_executor(self: &Arc<Self>) -> LocalExecutor {
        let session_id = self.next_session_id();
        // Local tasks can be woken by a platform timer after the application has begun tearing
        // down. Keep the dispatch destination alive until every runnable has either reached the
        // main-thread queue or been handled by the platform's closed-queue fallback. Dropping a
        // !Send local runnable on the timer thread is never sound.
        let scheduler = self.clone();
        LocalExecutor::new(session_id, self.clone(), move |runnable| {
            scheduler.schedule_local(session_id, runnable);
        })
    }

    fn next_session_id(&self) -> SessionId {
        SessionId::new(self.next_session_id.fetch_add(1, Ordering::SeqCst))
    }

    #[cfg(feature = "profiler")]
    pub(crate) fn foreground_runnable_counter(
        &self,
    ) -> crate::profiler::journal::ForegroundRunnableCounter {
        self.foreground_runnables.clone()
    }
}

impl Scheduler for PlatformScheduler {
    #[cfg(not(target_family = "wasm"))]
    fn block(
        &self,
        _session_id: Option<SessionId>,
        mut future: Pin<&mut dyn Future<Output = ()>>,
        timeout: Option<Duration>,
    ) -> bool {
        use waker_fn::waker_fn;
        let deadline = timeout.map(|t| Instant::now() + t);
        let parker = parking::Parker::new();
        let unparker = parker.unparker();
        let waker = waker_fn(move || {
            unparker.unpark();
        });
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
            return true;
        }

        let park_deadline = |deadline: Instant| {
            // Timer expirations are only delivered every ~15.6 milliseconds by default on Windows.
            // We increase the resolution during this wait so that short timeouts stay reasonably short.
            let _timer_guard = self.dispatcher.increase_timer_resolution();
            parker.park_deadline(deadline)
        };

        loop {
            match deadline {
                Some(deadline) if !park_deadline(deadline) && deadline <= Instant::now() => {
                    return false;
                }
                Some(_) => (),
                None => parker.park(),
            }
            if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
                break true;
            }
        }
    }

    fn schedule_local(&self, _session_id: SessionId, runnable: Runnable<RunnableMeta>) {
        #[cfg(feature = "profiler")]
        self.foreground_runnables.queued();
        if !self
            .dispatcher
            .dispatch_on_main_thread(runnable, Priority::default())
        {
            #[cfg(feature = "profiler")]
            self.foreground_runnables.finished();
        }
    }

    fn schedule_background_with_priority(
        &self,
        runnable: Runnable<RunnableMeta>,
        priority: Priority,
    ) {
        self.dispatcher.dispatch(runnable, priority);
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        self.dispatcher.spawn_realtime(f);
    }

    #[track_caller]
    fn timer(&self, duration: Duration) -> Timer {
        let (tx, rx) = oneshot::channel();
        let dispatcher = self.dispatcher.clone();

        // Create a runnable that will send the completion signal
        let location = std::panic::Location::caller();
        let (runnable, _task) = async_task::Builder::new()
            .metadata(RunnableMeta {
                location,
                spawned: open_gpui_scheduler::SpawnTime(Instant::now()),
            })
            .spawn(
                move |_| async move {
                    let _ = tx.send(());
                },
                move |runnable| {
                    dispatcher.dispatch_after(duration, runnable);
                },
            );
        runnable.schedule();

        Timer::new(rx)
    }

    fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    fn spawn_dedicated(
        self: Arc<Self>,
        f: Box<
            dyn FnOnce(
                    LocalExecutor,
                )
                    -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + 'static>>
                + Send
                + 'static,
        >,
    ) -> Task<Box<dyn Any + Send + Sync>> {
        let session_id = self.next_session_id();
        spawn_dedicated_thread(session_id, self, move |executor| f(executor))
    }

    fn as_test(&self) -> Option<&TestScheduler> {
        None
    }
}

/// A production clock that uses the platform dispatcher's time.
struct PlatformClock {
    dispatcher: Arc<dyn PlatformDispatcher>,
}

impl Clock for PlatformClock {
    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now(&self) -> Instant {
        self.dispatcher.now()
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    #[cfg(feature = "profiler")]
    use crate::{
        ForegroundExecutor,
        profiler::journal::{self, ForegroundJournalEntry, IntervalBoundary},
    };
    use crate::{PriorityQueueReceiver, RunnableVariant};
    #[cfg(feature = "profiler")]
    use futures::FutureExt;
    use open_gpui_scheduler::BackgroundExecutor;
    use parking_lot::Mutex;
    use std::time::Instant as StdInstant;

    // `spawn_dedicated` shouldn't touch the platform dispatcher at all;
    // panicking on every method ensures the test catches it if it does.
    struct SmokeDispatcher;

    impl PlatformDispatcher for SmokeDispatcher {
        fn is_main_thread(&self) -> bool {
            false
        }
        fn dispatch(&self, _runnable: RunnableVariant, _priority: Priority) {
            panic!("SmokeDispatcher should not be asked to dispatch in this test");
        }
        fn dispatch_on_main_thread(&self, _runnable: RunnableVariant, _priority: Priority) -> bool {
            panic!("SmokeDispatcher does not implement a main thread");
        }
        fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
            panic!("SmokeDispatcher does not implement timers");
        }
        fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
            panic!("SmokeDispatcher does not implement realtime");
        }
    }

    struct MainQueueDispatcher {
        sender: crate::PriorityQueueSender<RunnableVariant>,
        background: Mutex<Vec<RunnableVariant>>,
        delayed: Mutex<Vec<(Duration, RunnableVariant)>>,
        idle_timeouts: Mutex<Vec<Option<Duration>>>,
    }

    impl MainQueueDispatcher {
        fn new(sender: crate::PriorityQueueSender<RunnableVariant>) -> Self {
            Self {
                sender,
                background: Mutex::new(Vec::new()),
                delayed: Mutex::new(Vec::new()),
                idle_timeouts: Mutex::new(Vec::new()),
            }
        }
    }

    impl PlatformDispatcher for MainQueueDispatcher {
        fn is_main_thread(&self) -> bool {
            true
        }

        fn dispatch(&self, runnable: RunnableVariant, _priority: Priority) {
            self.background.lock().push(runnable);
        }

        fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
            self.sender
                .send(priority, runnable)
                .expect("main-thread runnable queue should remain connected");
            true
        }

        fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
            self.delayed.lock().push((duration, runnable));
        }

        fn dispatch_on_main_thread_when_idle(
            &self,
            runnable: RunnableVariant,
            timeout: Option<Duration>,
        ) -> bool {
            self.idle_timeouts.lock().push(timeout);
            self.dispatch_on_main_thread(runnable, Priority::Low)
        }

        fn idle_time_remaining(&self) -> Option<Duration> {
            Some(Duration::from_millis(2))
        }

        fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
            panic!("MainQueueDispatcher does not implement realtime work");
        }
    }

    #[cfg(feature = "profiler")]
    struct ClosingMainQueueDispatcher {
        sender: crate::PriorityQueueSender<RunnableVariant>,
        main_thread: std::thread::ThreadId,
        rejected_on: Mutex<Vec<std::thread::ThreadId>>,
    }

    #[cfg(feature = "profiler")]
    impl PlatformDispatcher for ClosingMainQueueDispatcher {
        fn is_main_thread(&self) -> bool {
            std::thread::current().id() == self.main_thread
        }

        fn dispatch(&self, _runnable: RunnableVariant, _priority: Priority) {
            panic!("ClosingMainQueueDispatcher does not implement background dispatch");
        }

        fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
            if let Err(runnable) = self.sender.send(priority, runnable) {
                self.rejected_on.lock().push(std::thread::current().id());
                // Match native closed-queue handling: a worker must not drop a !Send future.
                std::mem::forget(runnable);
                false
            } else {
                true
            }
        }

        fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
            panic!("ClosingMainQueueDispatcher does not implement timers");
        }

        fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
            panic!("ClosingMainQueueDispatcher does not implement realtime work");
        }
    }

    fn run_runnable(runnable: RunnableVariant) {
        let metadata = runnable.metadata();
        crate::profiler::update_running_task(metadata.spawned, metadata.location);
        runnable.run();
        crate::profiler::save_task_timing();
    }

    fn run_next_main_runnable(receiver: &mut PriorityQueueReceiver<RunnableVariant>) {
        run_runnable(
            receiver
                .try_pop()
                .expect("main-thread queue should remain connected")
                .expect("a main-thread runnable should be queued"),
        );
    }

    #[cfg(feature = "profiler")]
    fn retain_foreground_activity() {
        let now = Instant::now();
        journal::record_input(journal::InputTiming {
            kind: "test",
            start: now,
            end: now,
            caused_invalidation: false,
        });
    }

    #[cfg(feature = "profiler")]
    fn has_idle_boundary(entries: &[ForegroundJournalEntry]) -> bool {
        entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
            )
        })
    }

    #[cfg(feature = "profiler")]
    fn assert_closed_queue_late_wake_allows_reconstructed_foreground_idle(idle: bool) {
        // A failed regression intentionally leaves a ghost count. Isolate its TLS without
        // resetting the counter or allowing it to affect another test on a reused harness thread.
        std::thread::spawn(move || {
            use std::{cell::Cell, rc::Rc, thread};

            struct LocalDropProbe {
                main_thread: thread::ThreadId,
                dropped: Rc<Cell<bool>>,
            }

            impl Drop for LocalDropProbe {
                fn drop(&mut self) {
                    assert_eq!(thread::current().id(), self.main_thread);
                    self.dropped.set(true);
                }
            }

            let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
            let mut collector = journal.collector();
            let main_thread = thread::current().id();
            let (sender, mut receiver) = PriorityQueueReceiver::new();
            let dispatcher = Arc::new(ClosingMainQueueDispatcher {
                sender,
                main_thread,
                rejected_on: Mutex::new(Vec::new()),
            });
            let executor = ForegroundExecutor::new(dispatcher.clone());
            assert!(!executor.uses_test_dispatcher());

            let dropped = Rc::new(Cell::new(false));
            let polls = Rc::new(Cell::new(0));
            let probe = LocalDropProbe {
                main_thread,
                dropped: dropped.clone(),
            };
            let (resume_tx, resume_rx) = oneshot::channel::<()>();
            let future = {
                let polls = polls.clone();
                async move {
                    let _probe = probe;
                    polls.set(polls.get() + 1);
                    resume_rx
                        .await
                        .expect("the worker should deliver the late wake");
                    polls.set(polls.get() + 1);
                }
            };
            let task = if idle {
                executor.spawn_when_idle(Some(Duration::from_millis(25)), future)
            } else {
                executor.spawn(future)
            };
            task.detach();

            retain_foreground_activity();
            run_next_main_runnable(&mut receiver);
            assert_eq!(polls.get(), 1);
            assert!(!dropped.get());
            assert!(has_idle_boundary(&collector.collect_unseen().entries));
            assert!(receiver.try_pop().unwrap().is_none());
            drop(executor);
            drop(receiver);

            let worker_thread = thread::spawn(move || {
                resume_tx
                    .send(())
                    .expect("the detached local task must remain alive");
                thread::current().id()
            })
            .join()
            .expect("rejecting a late wake must not drop the local future on the worker");
            assert_ne!(worker_thread, main_thread);
            assert_eq!(*dispatcher.rejected_on.lock(), vec![worker_thread]);
            assert_eq!(polls.get(), 1, "rejected work must not be polled");
            assert!(
                !dropped.get(),
                "native rejection must forget the local future"
            );

            // Reconstruct on the same foreground thread, sharing the journal and counter.
            let (sender, mut receiver) = PriorityQueueReceiver::new();
            let executor = ForegroundExecutor::new(Arc::new(MainQueueDispatcher::new(sender)));
            let completed = executor.spawn(async { 42 });
            retain_foreground_activity();
            run_next_main_runnable(&mut receiver);
            assert_eq!(completed.now_or_never(), Some(42));
            assert!(receiver.try_pop().unwrap().is_none());
            assert!(
                has_idle_boundary(&collector.collect_unseen().entries),
                "a rejected late wake (idle={idle}) must not keep the reconstructed foreground busy"
            );
        })
        .join()
        .expect("closed-queue reconstruction must preserve foreground idle boundaries");
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn closed_queue_foreground_late_wake_allows_reconstructed_foreground_idle() {
        assert_closed_queue_late_wake_allows_reconstructed_foreground_idle(false);
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn closed_queue_idle_late_wake_allows_reconstructed_foreground_idle() {
        assert_closed_queue_late_wake_allows_reconstructed_foreground_idle(true);
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn foreground_runnables_delay_idle_until_all_runnables_finish() {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut collector = journal.collector();
        let (sender, mut receiver) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(MainQueueDispatcher::new(sender));
        let executor = ForegroundExecutor::new(dispatcher);
        assert!(!executor.uses_test_dispatcher());

        let (resume_tx, resume_rx) = oneshot::channel::<()>();
        let first = executor.spawn(async move {
            resume_rx
                .await
                .expect("worker should resume the foreground");
            1
        });
        let second = executor.spawn_with_priority(Priority::Low, async { 2 });
        let cancelled: Task<()> = executor.spawn(async {
            panic!("a cancelled foreground future must not be polled");
        });
        drop(cancelled);
        retain_foreground_activity();

        run_next_main_runnable(&mut receiver);
        assert!(!has_idle_boundary(&collector.collect_unseen().entries));
        std::thread::spawn(move || resume_tx.send(()))
            .join()
            .expect("worker wake should complete")
            .expect("foreground task should still be waiting");
        run_next_main_runnable(&mut receiver);
        assert!(!has_idle_boundary(&collector.collect_unseen().entries));
        run_next_main_runnable(&mut receiver);
        assert!(!has_idle_boundary(&collector.collect_unseen().entries));
        run_next_main_runnable(&mut receiver);
        assert!(has_idle_boundary(&collector.collect_unseen().entries));
        assert!(receiver.try_pop().unwrap().is_none());
        assert_eq!(first.now_or_never(), Some(1));
        assert_eq!(second.now_or_never(), Some(2));
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn unqueued_main_thread_work_preserves_queued_foreground_runnables() {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut collector = journal.collector();
        let (sender, mut receiver) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(MainQueueDispatcher::new(sender));
        let scheduler = Arc::new(PlatformScheduler::new(dispatcher.clone()));
        let foreground = scheduler.foreground_executor();
        let background = BackgroundExecutor::new(scheduler);
        let first = foreground.spawn(async { 1 });
        let second = foreground.spawn(async { 2 });
        let unqueued = background.spawn(async {
            std::thread::sleep(journal::TASK_POLL_FLOOR);
            3
        });

        let runnable = dispatcher.background.lock().pop().unwrap();
        let metadata = runnable.metadata();
        let location = metadata.location;
        {
            let _timing =
                crate::profiler::task_timing_unqueued(metadata.spawned, metadata.location);
            runnable.run();
        }
        assert_eq!(unqueued.now_or_never(), Some(3));
        let entries = collector.collect_unseen().entries;
        assert!(entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::Event(journal::ForegroundEvent::TaskPoll(timing))
                    if timing.location == location
            )
        }));
        assert!(!has_idle_boundary(&entries));

        run_next_main_runnable(&mut receiver);
        assert!(!has_idle_boundary(&collector.collect_unseen().entries));
        run_next_main_runnable(&mut receiver);
        assert!(has_idle_boundary(&collector.collect_unseen().entries));
        assert!(receiver.try_pop().unwrap().is_none());
        assert_eq!(first.now_or_never(), Some(1));
        assert_eq!(second.now_or_never(), Some(2));
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn idle_dispatch_counts_each_poll_once_and_preserves_timeout() {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut collector = journal.collector();
        let (sender, mut receiver) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(MainQueueDispatcher::new(sender));
        let executor = ForegroundExecutor::new(dispatcher.clone());
        let timeout = Some(Duration::from_millis(25));
        assert_eq!(
            executor.idle_time_remaining(),
            Some(Duration::from_millis(2))
        );

        let mut woken = false;
        let task = executor.spawn_when_idle(
            timeout,
            futures::future::poll_fn(move |cx| {
                if !woken {
                    woken = true;
                    let waker = cx.waker().clone();
                    std::thread::spawn(move || waker.wake())
                        .join()
                        .expect("worker idle wake should complete");
                    std::task::Poll::Pending
                } else {
                    std::task::Poll::Ready(42)
                }
            }),
        );
        retain_foreground_activity();
        assert_eq!(*dispatcher.idle_timeouts.lock(), vec![timeout]);

        run_next_main_runnable(&mut receiver);
        assert!(!has_idle_boundary(&collector.collect_unseen().entries));
        assert_eq!(*dispatcher.idle_timeouts.lock(), vec![timeout, timeout]);
        run_next_main_runnable(&mut receiver);
        assert!(has_idle_boundary(&collector.collect_unseen().entries));
        assert!(receiver.try_pop().unwrap().is_none());
        assert_eq!(task.now_or_never(), Some(42));
    }

    #[cfg(feature = "profiler")]
    #[test]
    fn delayed_background_work_does_not_hold_foreground_busy() {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut collector = journal.collector();
        let (sender, mut receiver) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(MainQueueDispatcher::new(sender));
        let scheduler = Arc::new(PlatformScheduler::new(dispatcher.clone()));
        let foreground = scheduler.foreground_executor();
        let background = crate::BackgroundExecutor::new(dispatcher.clone());
        let delay = Duration::from_secs(1);
        let timer = background.timer(delay);
        let task = foreground.spawn(async move {
            timer.await;
            42
        });
        retain_foreground_activity();

        run_next_main_runnable(&mut receiver);
        assert!(has_idle_boundary(&collector.collect_unseen().entries));
        assert!(receiver.try_pop().unwrap().is_none());

        let timer_wait = dispatcher.background.lock().pop().unwrap();
        std::thread::spawn(move || run_runnable(timer_wait))
            .join()
            .expect("background timer wait should park");
        let (scheduled_delay, delayed) = dispatcher.delayed.lock().pop().unwrap();
        assert_eq!(scheduled_delay, delay);
        std::thread::spawn(move || run_runnable(delayed))
            .join()
            .expect("delayed timer runnable should complete");
        let timer_wake = dispatcher.background.lock().pop().unwrap();
        std::thread::spawn(move || run_runnable(timer_wake))
            .join()
            .expect("background timer completion should wake the foreground");

        retain_foreground_activity();
        run_next_main_runnable(&mut receiver);
        assert!(has_idle_boundary(&collector.collect_unseen().entries));
        assert!(receiver.try_pop().unwrap().is_none());
        assert!(dispatcher.background.lock().is_empty());
        assert!(dispatcher.delayed.lock().is_empty());
        assert_eq!(task.now_or_never(), Some(42));
    }

    #[test]
    fn foreground_executor_retains_dispatch_authority_until_local_task_finishes() {
        use std::{cell::Cell, rc::Rc, thread};

        let (sender, mut receiver) = PriorityQueueReceiver::new();
        let scheduler = Arc::new(PlatformScheduler::new(Arc::new(MainQueueDispatcher::new(
            sender,
        ))));
        let weak_scheduler = Arc::downgrade(&scheduler);
        let executor = scheduler.foreground_executor();
        let (resume_tx, resume_rx) = futures::channel::oneshot::channel::<()>();
        let dropped_on_main = Rc::new(Cell::new(false));

        struct LocalDropProbe {
            dropped: Rc<Cell<bool>>,
            main_thread: thread::ThreadId,
        }

        impl Drop for LocalDropProbe {
            fn drop(&mut self) {
                assert_eq!(thread::current().id(), self.main_thread);
                self.dropped.set(true);
            }
        }

        executor
            .spawn({
                let dropped = Rc::clone(&dropped_on_main);
                async move {
                    let _probe = LocalDropProbe {
                        dropped,
                        main_thread: thread::current().id(),
                    };
                    resume_rx.await.expect("resume sender should remain live");
                }
            })
            .detach();

        run_next_main_runnable(&mut receiver);
        drop(executor);
        drop(scheduler);
        assert!(
            weak_scheduler.upgrade().is_some(),
            "the local task must retain its dispatch destination while it can still wake"
        );

        let wake = thread::spawn(move || resume_tx.send(()));
        assert_eq!(
            wake.join().expect("background wake must not panic"),
            Ok(()),
            "the pending local task must still accept its wake"
        );
        run_next_main_runnable(&mut receiver);

        assert!(dropped_on_main.get());
        assert!(
            weak_scheduler.upgrade().is_none(),
            "completed local work must release the retained scheduler"
        );
    }

    #[test]
    fn dedicated_executor_tasks_share_one_thread() {
        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let dedicated = open_gpui_scheduler::DedicatedExecutor::new(&background);

        let first = dedicated.spawn(async { std::thread::current().id() });
        let second = dedicated.spawn(async { std::thread::current().id() });

        let first = futures::executor::block_on(first);
        let second = futures::executor::block_on(second);

        assert_eq!(first, second, "tasks must share the dedicated thread");
        assert_ne!(
            first,
            std::thread::current().id(),
            "dedicated tasks must not run on the spawning thread"
        );
    }

    #[test]
    fn spawn_dedicated_applies_disposition_before_root_delivery() {
        use std::sync::mpsc;

        struct DropSignal(mpsc::Sender<std::thread::ThreadId>);

        impl Drop for DropSignal {
            fn drop(&mut self) {
                let _ = self.0.send(std::thread::current().id());
            }
        }

        for detach in [false, true] {
            let scheduler = Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher)));
            let weak_scheduler = Arc::downgrade(&scheduler);
            let session_id = scheduler.next_session_id();
            let (task_tx, task_rx) = mpsc::channel();
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (dropped_tx, dropped_rx) = mpsc::channel();
            let (completed_tx, completed_rx) = mpsc::channel();
            let timeout = Duration::from_secs(5);

            let caller = std::thread::spawn(move || {
                let task = open_gpui_scheduler::spawn_dedicated_thread(
                    session_id,
                    scheduler,
                    move |_executor| {
                        let drop_signal = DropSignal(dropped_tx);
                        entered_tx
                            .send(std::thread::current().id())
                            .expect("the test must observe the root factory");
                        release_rx
                            .recv_timeout(timeout)
                            .expect("the caller must release the root factory");
                        async move {
                            let _drop_signal = drop_signal;
                            completed_tx
                                .send(())
                                .expect("the test must observe completion");
                        }
                    },
                );
                task_tx.send(task).expect("the test must receive the task");
            });

            let dedicated_thread = entered_rx
                .recv_timeout(timeout)
                .expect("the dedicated root factory must start");
            let task = task_rx
                .recv_timeout(timeout)
                .expect("spawning must return before the root factory is released");
            assert!(!task.is_ready());
            if detach {
                task.detach();
            } else {
                drop(task);
            }
            release_tx
                .send(())
                .expect("the root factory must be waiting");
            caller.join().expect("the spawning thread must finish");

            assert_eq!(
                dropped_rx
                    .recv_timeout(timeout)
                    .expect("the root future must be destroyed"),
                dedicated_thread
            );
            if detach {
                completed_rx
                    .recv_timeout(timeout)
                    .expect("detached work must complete");
            } else {
                assert!(matches!(
                    completed_rx.recv_timeout(timeout),
                    Err(mpsc::RecvTimeoutError::Disconnected)
                ));
            }
            let deadline = StdInstant::now() + timeout;
            while weak_scheduler.upgrade().is_some() && StdInstant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(weak_scheduler.upgrade().is_none());
        }
    }

    #[test]
    fn spawn_dedicated_runs_on_a_real_separate_thread() {
        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let started = StdInstant::now();
        let task = background.spawn_dedicated(|_executor| async move {
            // A genuine blocking syscall on the dedicated thread. If
            // `spawn_dedicated` were running the future on any shared
            // executor, this would stall that executor.
            let thread_id_before = std::thread::current().id();
            std::thread::sleep(Duration::from_millis(50));
            let thread_id_after = std::thread::current().id();
            assert_eq!(thread_id_before, thread_id_after);
            (thread_id_before, "slept")
        });
        let (dedicated_thread_id, message) = futures::executor::block_on(task);
        let elapsed = started.elapsed();
        assert_eq!(message, "slept");
        assert_ne!(
            dedicated_thread_id,
            std::thread::current().id(),
            "dedicated future ran on the test thread"
        );
        assert!(
            elapsed >= Duration::from_millis(40),
            "expected the dedicated thread to genuinely sleep, elapsed = {:?}",
            elapsed
        );
    }

    #[test]
    fn spawn_dedicated_returns_not_send_future_output() {
        // The whole point of `spawn_dedicated` is that the future can be
        // `!Send`. Constructing one with `Rc<RefCell<_>>` ensures the
        // signature actually permits it.
        use std::cell::RefCell;
        use std::rc::Rc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let task = background.spawn_dedicated(|_executor| async move {
            let state = Rc::new(RefCell::new(0_i32));
            for _ in 0..3 {
                *state.borrow_mut() += 1;
            }
            *state.borrow()
        });
        let output = futures::executor::block_on(task);
        assert_eq!(output, 3);
    }

    #[test]
    fn spawn_dedicated_dropping_task_cancels_future() {
        use parking_lot::Mutex;
        use std::sync::mpsc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));

        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (after_park_tx, after_park_rx) = mpsc::channel::<()>();
        let observed_post_await_write = Arc::new(Mutex::new(false));

        let task = {
            let observed_post_await_write = observed_post_await_write.clone();
            background.spawn_dedicated(move |_executor| async move {
                // Announce that the future is live on the dedicated thread.
                started_tx
                    .send(())
                    .expect("started signal must be received");
                // Park forever. Dropping the `Task` must cancel us here so
                // the code below this `await` never runs.
                futures::future::pending::<()>().await;
                *observed_post_await_write.lock() = true;
                after_park_tx
                    .send(())
                    .expect("after-park signal must be received");
            })
        };

        // Wait until the dedicated future is actually parked at the await.
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("dedicated future failed to start");

        // Drop the root Task: this must cancel the future.
        drop(task);

        // If cancellation works, the future never advances past `pending`,
        // so this recv must time out.
        assert!(
            after_park_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "dedicated future advanced past the await after its Task was dropped"
        );
        assert!(
            !*observed_post_await_write.lock(),
            "dedicated future ran code past the cancellation point"
        );
    }

    #[test]
    fn spawn_dedicated_thread_tears_down_after_work_completes() {
        use std::sync::mpsc;

        // Fires from `Drop` so we observe teardown of the dedicated future's
        // captured state on whichever thread runs its destructor.
        struct DropSignal {
            tx: Option<mpsc::Sender<std::thread::ThreadId>>,
        }
        impl Drop for DropSignal {
            fn drop(&mut self) {
                if let Some(tx) = self.tx.take() {
                    let _ = tx.send(std::thread::current().id());
                }
            }
        }

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let (started_tx, started_rx) = mpsc::channel::<std::thread::ThreadId>();
        let (drop_tx, drop_rx) = mpsc::channel::<std::thread::ThreadId>();

        let task = background.spawn_dedicated(move |_executor| async move {
            // Captured by the future's state. When the future completes and
            // its state is dropped on the dedicated thread, this guard's
            // `Drop` fires and reports the thread id it ran on.
            let _guard = DropSignal { tx: Some(drop_tx) };
            started_tx
                .send(std::thread::current().id())
                .expect("started signal must be received");
            // Future returns immediately. The dedicated thread should then
            // drop the future (firing _guard), exit the recv loop, and exit.
        });

        let dedicated_thread_id = started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("dedicated future failed to start");
        assert_ne!(
            dedicated_thread_id,
            std::thread::current().id(),
            "dedicated future ran on the test thread"
        );

        // Drive the root task to completion so its body finishes.
        futures::executor::block_on(task);

        // The guard's drop runs from the dedicated thread as it tears down
        // the future's captured state. If the executor/recv-loop were
        // keeping the future alive past task completion, this would hang.
        let drop_thread_id = drop_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("dedicated future's captured state was not dropped after task completion");
        assert_eq!(
            drop_thread_id, dedicated_thread_id,
            "dedicated future's captured state must be dropped on the dedicated thread, not elsewhere"
        );
    }

    #[test]
    fn spawn_dedicated_detached_child_outlives_root() {
        use std::sync::mpsc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));

        // `gate_rx` lets the detached child park until the test explicitly
        // releases it — after we've already observed the root completing.
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let (child_done_tx, child_done_rx) = mpsc::channel::<std::thread::ThreadId>();

        let task = background.spawn_dedicated(move |executor| async move {
            executor
                .spawn(async move {
                    // Blocking on `recv` is normally wrong inside an
                    // executor, but the dedicated thread is exclusive to
                    // this session, so blocking the only future on it is
                    // fine — this is the property `spawn_dedicated` is
                    // designed to provide.
                    gate_rx
                        .recv()
                        .expect("gate sender dropped before child resumed");
                    child_done_tx
                        .send(std::thread::current().id())
                        .expect("child_done receiver dropped");
                })
                .detach();
            // Root finishes here. The detached child must keep the
            // dedicated thread alive until it completes.
        });

        futures::executor::block_on(task);

        // Negative assertion: the child has not finished, because the gate
        // hasn't been released yet.
        assert!(
            child_done_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "detached child finished before being released"
        );

        // Release the gate. The detached child should now complete on the
        // dedicated thread.
        gate_tx.send(()).expect("gate receiver dropped");

        let child_thread_id = child_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("detached child failed to complete after gate was released");
        assert_ne!(
            child_thread_id,
            std::thread::current().id(),
            "detached child ran on the test thread instead of the dedicated thread"
        );
    }
}
