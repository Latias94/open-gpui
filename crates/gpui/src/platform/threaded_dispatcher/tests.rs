//! The eight upstream regressions are from Zed 635026fabc38440fce5e1767d125026a17b12012.
//! Fork regressions additionally cover admission, scoped timing, cancellation reentry and unwind.

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::*;
use crate::{BackgroundExecutor, ForegroundExecutor};

#[test]
fn is_idle_tracks_queued_work_but_ignores_undue_timers() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    assert!(dispatcher.is_idle());

    foreground.spawn(async {}).detach();
    assert!(!dispatcher.is_idle());
    dispatcher.run_until_idle();
    assert!(dispatcher.is_idle());

    let background = BackgroundExecutor::new(dispatcher.clone());
    let timer = background.timer(Duration::from_secs(60));
    // The timer future's initial poll runs on a worker thread; wait for
    // it so only the armed, not-yet-due timer remains.
    dispatcher.run_until_idle();
    assert!(
        dispatcher.is_idle(),
        "a timer that is not due yet should not count as pending work"
    );
    drop(timer);
    dispatcher.cancel_pending_timers();
    dispatcher.run_until_idle();
    assert!(dispatcher.is_idle());
}

#[test]
fn run_ready_main_tasks_does_not_wait_for_background_handoffs() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let foreground = ForegroundExecutor::new(dispatcher.clone());

    let (sender, receiver) = futures::channel::oneshot::channel();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    background
        .spawn(async move {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            sender.send(()).ok();
        })
        .detach();

    let completed = Arc::new(AtomicBool::new(false));
    foreground
        .spawn({
            let completed = completed.clone();
            async move {
                receiver.await.ok();
                completed.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(dispatcher.run_ready_main_tasks());
    assert!(!completed.load(Ordering::SeqCst));

    release_tx.send(()).unwrap();
    dispatcher.run_until_idle();
    assert!(completed.load(Ordering::SeqCst));
}

#[test]
fn run_until_idle_completes_background_to_main_handoffs() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let foreground = ForegroundExecutor::new(dispatcher.clone());

    let (sender, receiver) = futures::channel::oneshot::channel();
    background
        .spawn(async move {
            thread::sleep(Duration::from_millis(10));
            sender.send(()).ok();
        })
        .detach();

    let completed = Arc::new(AtomicBool::new(false));
    foreground
        .spawn({
            let completed = completed.clone();
            async move {
                receiver.await.ok();
                completed.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    dispatcher.run_until_idle();
    assert!(completed.load(Ordering::SeqCst));
}

#[test]
fn run_until_waits_for_untracked_external_wakes() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let (sender, receiver) = futures::channel::oneshot::channel();
    let sender_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        sender
            .send(())
            .expect("foreground receiver should remain alive");
    });

    let completed = Arc::new(AtomicBool::new(false));
    foreground
        .spawn({
            let completed = completed.clone();
            async move {
                receiver
                    .await
                    .expect("external sender should deliver its wake");
                completed.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    dispatcher.run_until(|| completed.load(Ordering::SeqCst).then_some(()));
    sender_thread.join().expect("sender thread should finish");
    assert!(completed.load(Ordering::SeqCst));
}

#[test]
fn run_until_returns_at_readiness_despite_requeuing_main_work() {
    const REQUEUE_LIMIT: usize = 10_000;

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());

    // Mirrors main-thread work that yields and immediately re-queues
    // itself (e.g. an idle-time sweep): the main queue never drains until
    // such work finishes every iteration, so readiness must be observed
    // between runnables rather than only at quiescence.
    let iterations = Arc::new(AtomicUsize::new(0));
    foreground
        .spawn({
            let iterations = iterations.clone();
            async move {
                for _ in 0..REQUEUE_LIMIT {
                    iterations.fetch_add(1, Ordering::SeqCst);
                    yield_once().await;
                }
            }
        })
        .detach();

    let completed = Arc::new(AtomicBool::new(false));
    foreground
        .spawn({
            let completed = completed.clone();
            async move {
                completed.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    dispatcher.run_until(|| completed.load(Ordering::SeqCst).then_some(()));
    assert!(
        iterations.load(Ordering::SeqCst) < REQUEUE_LIMIT,
        "run_until should return at readiness instead of draining re-queued main work"
    );
    dispatcher.run_until_idle();
}

/// Completes after one re-schedule: the poll returns `Pending` and wakes
/// immediately, so the runnable re-enters the main queue.
fn yield_once() -> impl Future<Output = ()> {
    let mut yielded = false;
    std::future::poll_fn(move |poll_context| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            poll_context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
}

#[test]
fn run_ready_main_tasks_advances_requeuing_work_one_batch_per_call() {
    const REQUEUE_LIMIT: usize = 10_000;

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());

    let iterations = Arc::new(AtomicUsize::new(0));
    foreground
        .spawn({
            let iterations = iterations.clone();
            async move {
                for _ in 0..REQUEUE_LIMIT {
                    iterations.fetch_add(1, Ordering::SeqCst);
                    yield_once().await;
                }
            }
        })
        .detach();

    assert!(dispatcher.run_ready_main_tasks());
    assert_eq!(iterations.load(Ordering::SeqCst), 1);
    assert!(dispatcher.run_ready_main_tasks());
    assert_eq!(iterations.load(Ordering::SeqCst), 2);
    dispatcher.run_until_idle();
}

#[test]
fn timers_fire_in_real_time() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher);

    let fired = Arc::new(AtomicBool::new(false));
    let timer = background.timer(Duration::from_millis(10));
    background
        .spawn({
            let fired = fired.clone();
            async move {
                timer.await;
                fired.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    let deadline = Instant::now() + Duration::from_secs(10);
    while !fired.load(Ordering::SeqCst) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(fired.load(Ordering::SeqCst));
}

#[test]
fn cancel_pending_timers_wakes_waiters_without_waiting_for_deadline() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());

    let fired = Arc::new(AtomicBool::new(false));
    let timer = background.timer(Duration::from_secs(10));
    background
        .spawn({
            let fired = fired.clone();
            async move {
                timer.await;
                fired.store(true, Ordering::SeqCst);
            }
        })
        .detach();

    dispatcher.run_until_idle();
    assert_eq!(dispatcher.cancel_pending_timers(), 1);
    dispatcher.run_until_idle();

    assert!(fired.load(Ordering::SeqCst));
    assert_eq!(dispatcher.cancel_pending_timers(), 0);
}

#[test]
fn run_until_idle_rechecks_main_work_after_an_empty_turn() {
    use std::cell::Cell;

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let completed = std::rc::Rc::new(Cell::new(false));
    let mut first_turn = true;
    dispatcher.run_until_idle_with(|| {
        if first_turn {
            first_turn = false;
            let completed = completed.clone();
            foreground
                .spawn(async move { completed.set(true) })
                .detach();
            false
        } else {
            dispatcher.run_ready_main_tasks()
        }
    });
    assert!(
        completed.get(),
        "a wake before the idle wait must not be lost"
    );
}

#[test]
fn ready_turn_checks_between_polls_and_allows_reentrant_after_poll() {
    use std::{cell::Cell, rc::Rc};

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let polls = Rc::new(Cell::new(0));
    for _ in 0..2 {
        let polls = polls.clone();
        foreground
            .spawn(async move { polls.set(polls.get() + 1) })
            .detach();
    }

    let after_polls = Cell::new(0);
    assert!(dispatcher.run_ready_main_tasks_with(
        || polls.get() == 0,
        || {
            assert_eq!(polls.get(), 1);
            assert_eq!(dispatcher.main_receiver.lock().len(), 1);
            let polls = polls.clone();
            foreground
                .spawn(async move { polls.set(polls.get() + 1) })
                .detach();
            after_polls.set(after_polls.get() + 1);
        },
    ));
    assert_eq!(polls.get(), 1);
    assert_eq!(after_polls.get(), 1);
    assert_eq!(dispatcher.main_receiver.lock().len(), 2);
    dispatcher.run_until_idle();
    assert_eq!(polls.get(), 3);
}

#[test]
fn run_until_with_frames_returns_before_unrelated_frame_work() {
    use std::{cell::Cell, rc::Rc};

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let completed = Rc::new(Cell::new(false));
    foreground
        .spawn({
            let completed = completed.clone();
            async move { completed.set(true) }
        })
        .detach();
    let frames = Cell::new(0);
    assert_eq!(
        dispatcher.run_until_with_frames(
            || completed.get().then_some(42),
            || {
                frames.set(frames.get() + 1);
                true
            },
        ),
        42,
    );
    assert_eq!(frames.get(), 0);

    completed.set(false);
    let mut first_frame = true;
    dispatcher.run_until_with_frames(
        || completed.get().then_some(()),
        || {
            assert!(first_frame);
            first_frame = false;
            let completed = completed.clone();
            foreground
                .spawn(async move { completed.set(true) })
                .detach();
            false
        },
    );
    assert!(
        completed.get(),
        "main work queued before waiting must be observed"
    );
}

#[track_caller]
fn detached_runnable(future: impl Future<Output = ()> + Send + 'static) -> RunnableVariant {
    let (runnable, task) = async_task::Builder::new()
        .metadata(crate::RunnableMeta {
            location: std::panic::Location::caller(),
            spawned: open_gpui_scheduler::SpawnTime(open_gpui_scheduler::Instant::now()),
        })
        .spawn(move |_| future, |_| {});
    task.detach();
    runnable
}

#[test]
fn cancel_pending_timers_drops_outside_the_lock_and_allows_rearming() {
    struct RearmOnDrop {
        dispatcher: Arc<ThreadedDispatcher>,
        replacement: Option<RunnableVariant>,
        observed_unlocked: Arc<AtomicBool>,
    }

    impl Drop for RearmOnDrop {
        fn drop(&mut self) {
            let unlocked = self
                .dispatcher
                .timers
                .state
                .try_lock_for(Duration::from_secs(5))
                .is_some();
            self.observed_unlocked.store(unlocked, Ordering::SeqCst);
            if unlocked {
                if let Some(replacement) = self.replacement.take() {
                    self.dispatcher
                        .dispatch_after(Duration::from_secs(60), replacement);
                }
            }
        }
    }

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let observed_unlocked = Arc::new(AtomicBool::new(false));
    let rearm = RearmOnDrop {
        dispatcher: dispatcher.clone(),
        replacement: Some(detached_runnable(async {})),
        observed_unlocked: observed_unlocked.clone(),
    };
    dispatcher.dispatch_after(
        Duration::from_secs(60),
        detached_runnable(async move { drop(rearm) }),
    );
    assert_eq!(dispatcher.cancel_pending_timers(), 1);
    assert!(
        observed_unlocked.load(Ordering::SeqCst),
        "timer cancellation must release the timer lock before dropping runnables"
    );
    assert_eq!(dispatcher.cancel_pending_timers(), 1);
    assert_eq!(dispatcher.cancel_pending_timers(), 0);
    dispatcher.run_until_idle();
}

#[test]
fn workers_run_in_parallel_and_remain_inflight_until_completion() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let mut releases = Vec::new();
    for _ in 0..2 {
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        releases.push(release_tx);
        let started_tx = started_tx.clone();
        background
            .spawn(async move {
                started_tx.send(thread::current().id()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            })
            .detach();
    }
    let first = started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let second = started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_ne!(first, second);
    assert_ne!(first, thread::current().id());
    assert_ne!(second, thread::current().id());
    assert!(!dispatcher.is_idle());
    assert_eq!(*dispatcher.idle.inflight.lock(), 2);
    for release in releases {
        release.send(()).unwrap();
    }
    dispatcher.run_until_idle();
    assert!(dispatcher.is_idle());
}

#[test]
fn background_admission_unwind_restores_inflight_count() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut dispatcher = ThreadedDispatcher::new();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            dispatcher.dispatch(detached_runnable(async {}), Priority::RealtimeAudio);
        }))
        .is_err()
    );
    assert!(dispatcher.is_idle());

    let (sender, receiver) = PriorityQueueReceiver::new();
    drop(receiver);
    dispatcher.background_sender = sender;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            dispatcher.dispatch(detached_runnable(async {}), Priority::Medium);
        }))
        .is_err()
    );
    assert!(dispatcher.is_idle());
}

#[test]
fn panicking_worker_and_timer_polls_release_inflight_counts() {
    let dispatcher = ThreadedDispatcher::new();
    for timer in [false, true] {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel::<()>();
        let runnable = detached_runnable(async move {
            entered_tx.send(()).unwrap();
            panic!("threaded dispatcher poll unwind");
        });
        if timer {
            dispatcher.dispatch_after(Duration::ZERO, runnable);
        } else {
            dispatcher.dispatch(runnable, Priority::Medium);
        }
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the runnable must enter its poll before panicking");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !dispatcher.is_idle() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(dispatcher.is_idle(), "{}", dispatcher.debug_state());
    }
}

#[test]
fn refused_main_and_idle_wakes_do_not_poll_or_drop_local_futures_off_thread() {
    for idle in [false, true] {
        thread::spawn(move || {
            use std::{cell::Cell, rc::Rc, task::Poll};

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

            #[cfg(feature = "profiler")]
            let (journal, _guard) =
                crate::profiler::journal::install_test_foreground_journal(64, 4);
            #[cfg(feature = "profiler")]
            let mut collector = journal.collector();
            let dispatcher = Arc::new(ThreadedDispatcher::new());
            let foreground = ForegroundExecutor::new(dispatcher.clone());
            let polls = Rc::new(Cell::new(0));
            let dropped = Rc::new(Cell::new(false));
            let probe = LocalDropProbe {
                main_thread: thread::current().id(),
                dropped: dropped.clone(),
            };
            let (waker_tx, waker_rx) = std::sync::mpsc::channel();
            let future = {
                let polls = polls.clone();
                std::future::poll_fn(move |cx| {
                    let _probe = &probe;
                    polls.set(polls.get() + 1);
                    waker_tx.send(cx.waker().clone()).unwrap();
                    Poll::<()>::Pending
                })
            };
            if idle {
                foreground.spawn_when_idle(None, future).detach();
            } else {
                foreground.spawn(future).detach();
            }
            assert!(dispatcher.run_ready_main_tasks());
            assert_eq!(polls.get(), 1);

            let (sender, receiver) = PriorityQueueReceiver::new();
            *dispatcher.main_receiver.lock() = receiver;
            drop(sender);
            let waker = waker_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            thread::spawn(move || waker.wake()).join().unwrap();
            assert_eq!(polls.get(), 1, "a refused runnable must never be polled");
            assert!(!dropped.get(), "a refused local runnable must be forgotten");
            assert!(!dispatcher.run_ready_main_tasks());

            #[cfg(feature = "profiler")]
            {
                let _ = collector.collect_unseen();
                record_foreground_activity();
                assert!(has_idle_boundary(&collector.collect_unseen().entries));
            }
        })
        .join()
        .unwrap();
    }
}

#[cfg(feature = "profiler")]
fn has_idle_boundary(entries: &[crate::profiler::journal::ForegroundJournalEntry]) -> bool {
    use crate::profiler::journal::{ForegroundJournalEntry, IntervalBoundary};

    entries.iter().any(|entry| {
        matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
        )
    })
}

#[cfg(feature = "profiler")]
fn record_foreground_activity() {
    use crate::profiler::journal;

    let _turn = journal::foreground_turn();
    let now = open_gpui_scheduler::Instant::now();
    journal::record_input(journal::InputTiming {
        kind: "test",
        start: now,
        end: now,
        caused_invalidation: false,
    });
}

#[cfg(feature = "profiler")]
#[test]
fn accepted_main_and_idle_polls_settle_foreground_counts_at_completion() {
    use futures::FutureExt;

    let (journal, _guard) = crate::profiler::journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let first = foreground.spawn(async { 1 });
    let idle = foreground.spawn_when_idle(Some(Duration::from_secs(60)), async { 2 });
    assert!(
        dispatcher
            .as_threaded()
            .is_some_and(|value| std::ptr::eq(value, &*dispatcher))
    );
    record_foreground_activity();
    assert!(!has_idle_boundary(&collector.collect_unseen().entries));

    assert!(dispatcher.run_one_main_task());
    assert!(!has_idle_boundary(&collector.collect_unseen().entries));
    assert!(dispatcher.run_one_main_task());
    assert!(has_idle_boundary(&collector.collect_unseen().entries));
    assert_eq!(first.now_or_never(), Some(1));
    assert_eq!(idle.now_or_never(), Some(2));
}

#[cfg(feature = "profiler")]
#[test]
fn nested_main_poll_unwind_restores_timing_and_settles_only_its_own_count() {
    use std::{cell::Cell, panic::AssertUnwindSafe, panic::catch_unwind, rc::Rc};

    let (journal, _guard) = crate::profiler::journal::install_test_foreground_journal(64, 4);
    let mut collector = journal.collector();
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let completed = Rc::new(Cell::new(false));
    let sentinel_completed = Rc::new(Cell::new(false));
    foreground
        .spawn({
            let dispatcher = dispatcher.clone();
            let foreground = foreground.clone();
            let completed = completed.clone();
            let sentinel_completed = sentinel_completed.clone();
            async move {
                let enclosing =
                    profiler::THREAD_TIMINGS.with(|timings| timings.lock().running.unwrap());
                foreground
                    .spawn(async { panic!("nested main poll unwind") })
                    .detach();
                foreground
                    .spawn(async move { sentinel_completed.set(true) })
                    .detach();
                assert!(catch_unwind(AssertUnwindSafe(|| dispatcher.run_one_main_task())).is_err());
                let restored =
                    profiler::THREAD_TIMINGS.with(|timings| timings.lock().running.unwrap());
                assert_eq!(restored.start, enclosing.start);
                assert_eq!(restored.spawned.0, enclosing.spawned.0);
                assert_eq!(restored.location, enclosing.location);
                completed.set(true);
            }
        })
        .detach();
    assert!(dispatcher.run_one_main_task());
    assert!(completed.get());
    assert!(!sentinel_completed.get());
    assert!(
        !has_idle_boundary(&collector.collect_unseen().entries),
        "the queued sentinel must keep the journal busy after the enclosing poll"
    );
    assert!(dispatcher.run_one_main_task());
    assert!(sentinel_completed.get());
    assert!(has_idle_boundary(&collector.collect_unseen().entries));
    assert!(profiler::THREAD_TIMINGS.with(|timings| timings.lock().running.is_none()));
}

#[cfg(feature = "profiler")]
#[test]
fn worker_and_timer_polls_preserve_unrelated_foreground_queue_counts() {
    use crate::profiler::journal;

    let dispatcher = ThreadedDispatcher::new();
    for timer in [false, true] {
        let (sender, receiver) = std::sync::mpsc::channel();
        let runnable = detached_runnable(async move {
            let journal = journal::install_foreground_journal();
            journal::begin_foreground_turn();
            let collector = journal.collector();
            let counter = journal::foreground_runnable_counter();
            counter.queued();
            record_foreground_activity();
            let running = profiler::THREAD_TIMINGS.with(|timings| timings.lock().running.unwrap());
            sender.send((collector, counter, running)).unwrap();
        });
        let metadata = runnable.metadata().clone();
        if timer {
            dispatcher.dispatch_after(Duration::ZERO, runnable);
        } else {
            dispatcher.dispatch(runnable, Priority::Medium);
        }
        dispatcher.run_until_idle();
        let (mut collector, counter, running) =
            receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        let _settle = open_gpui_util::defer(|| counter.finished());
        assert_eq!(running.spawned.0, metadata.spawned.0);
        assert_eq!(running.location, metadata.location);
        let entries = collector.collect_unseen();
        assert_eq!(entries.lost, 0);
        assert!(
            !has_idle_boundary(&entries.entries),
            "unqueued poll (timer={timer}) must not consume a foreground runnable: {entries:?}"
        );
    }
}

#[cfg(not(feature = "profiler"))]
#[test]
fn disabled_dispatcher_polls_do_not_register_task_timings() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let (sender, receiver) = std::sync::mpsc::channel();
    let observe = move || {
        sender
            .send((
                thread::current().id(),
                profiler::get_current_thread_timings(crate::TasksIncluded::CompletedAndRunning)
                    .timings
                    .is_empty(),
            ))
            .unwrap();
    };
    foreground
        .spawn({
            let observe = observe.clone();
            async move { observe() }
        })
        .detach();
    background
        .spawn({
            let observe = observe.clone();
            async move { observe() }
        })
        .detach();
    dispatcher.dispatch_after(Duration::ZERO, detached_runnable(async move { observe() }));
    dispatcher.run_until_idle();
    for _ in 0..3 {
        let (thread_id, empty) = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(empty);
        assert!(
            profiler::GLOBAL_THREAD_TIMINGS
                .lock()
                .iter()
                .all(|thread| thread.thread_id != thread_id)
        );
    }
    assert_eq!(std::mem::size_of::<profiler::TaskTimingGuard>(), 0);
}
