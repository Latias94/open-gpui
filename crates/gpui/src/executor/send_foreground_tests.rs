use super::*;
use crate::{TestDispatcher, ThreadedDispatcher};
use futures::{channel::oneshot, future};
use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
};

#[test]
fn send_foreground_executor_submits_from_worker() {
    let dispatcher = TestDispatcher::new(0);
    let foreground_executor = ForegroundExecutor::new(Arc::new(dispatcher.clone()));
    let main_thread = std::thread::current().id();
    let foreground_executor = foreground_executor.to_send();
    let task = std::thread::spawn(move || {
        foreground_executor.spawn(async move {
            assert_eq!(std::thread::current().id(), main_thread);
            42
        })
    })
    .join()
    .expect("worker must submit the foreground task");

    dispatcher.run_until_parked();
    assert_eq!(task.now_or_never(), Some(42));
}

#[test]
fn send_foreground_executor_repolls_on_main_after_original_executor_drops() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let weak_scheduler = Arc::downgrade(foreground.inner.scheduler());
    let send_executor = Arc::new(foreground.to_send());
    drop(foreground);

    let main_thread = thread::current().id();
    let polls = Arc::new(AtomicUsize::new(0));
    let (resume_tx, resume_rx) = oneshot::channel::<()>();
    let (send_executor, task) = thread::spawn({
        let polls = polls.clone();
        move || {
            assert_ne!(thread::current().id(), main_thread);
            let task = send_executor.spawn(async move {
                assert_eq!(thread::current().id(), main_thread);
                polls.fetch_add(1, Ordering::SeqCst);
                resume_rx.await.expect("worker must resume the task");
                assert_eq!(thread::current().id(), main_thread);
                polls.fetch_add(1, Ordering::SeqCst);
                Cell::new(42)
            });
            (send_executor, task)
        }
    })
    .join()
    .expect("worker submission must succeed");
    assert!(weak_scheduler.upgrade().is_some());
    drop(send_executor);
    assert!(weak_scheduler.upgrade().is_some());

    dispatcher.run_until_idle();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!task.is_ready());
    let task = thread::spawn(move || {
        resume_tx.send(()).expect("foreground task must be waiting");
        task
    })
    .join()
    .expect("worker wake must succeed");
    dispatcher.run_until_idle();
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert_eq!(task.now_or_never().map(|result| result.get()), Some(42));
    assert!(weak_scheduler.upgrade().is_none());
}

#[test]
fn worker_cancellation_drops_send_and_local_futures_on_main() {
    struct DropOnMain {
        main_thread: thread::ThreadId,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for DropOnMain {
        fn drop(&mut self) {
            assert_eq!(thread::current().id(), self.main_thread);
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let main_thread = thread::current().id();
    for poll_before_cancel in [false, true] {
        let send_executor = foreground.to_send();
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = DropOnMain {
            main_thread,
            dropped: dropped.clone(),
        };
        let task = thread::spawn(move || {
            send_executor.spawn(async move {
                let _probe = probe;
                assert_eq!(thread::current().id(), main_thread);
                future::pending::<()>().await;
            })
        })
        .join()
        .expect("worker submission must succeed");
        if poll_before_cancel {
            dispatcher.run_until_idle();
        }
        assert!(!dropped.load(Ordering::SeqCst));
        thread::spawn(move || drop(task))
            .join()
            .expect("worker cancellation must succeed");
        assert!(!dropped.load(Ordering::SeqCst));
        dispatcher.run_until_idle();
        assert!(dropped.load(Ordering::SeqCst));
    }

    let local_state = Rc::new(Cell::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let probe = DropOnMain {
        main_thread,
        dropped: dropped.clone(),
    };
    let task = foreground.spawn({
        let local_state = local_state.clone();
        async move {
            let _probe = probe;
            local_state.set(true);
            future::pending::<()>().await;
        }
    });
    dispatcher.run_until_idle();
    assert!(local_state.get());
    thread::spawn(move || drop(task))
        .join()
        .expect("worker cancellation of a local task must only enqueue its drop");
    assert!(!dropped.load(Ordering::SeqCst));
    dispatcher.run_until_idle();
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(Rc::strong_count(&local_state), 1);
}

#[cfg(feature = "profiler")]
mod accounting {
    use super::*;
    use crate::{
        PriorityQueueReceiver, PriorityQueueSender, RunnableVariant,
        profiler::journal::{self, ForegroundJournalEntry, IntervalBoundary},
    };
    use parking_lot::Mutex;
    use std::task::Poll;

    struct ClosingDispatcher {
        sender: PriorityQueueSender<RunnableVariant>,
        main_thread: thread::ThreadId,
        rejected_on: Mutex<Vec<thread::ThreadId>>,
    }

    impl PlatformDispatcher for ClosingDispatcher {
        fn is_main_thread(&self) -> bool {
            thread::current().id() == self.main_thread
        }

        fn dispatch(&self, _runnable: RunnableVariant, _priority: Priority) {
            panic!("ClosingDispatcher does not implement background dispatch");
        }

        fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
            if let Err(runnable) = self.sender.send(priority, runnable) {
                self.rejected_on.lock().push(thread::current().id());
                mem::forget(runnable);
                false
            } else {
                true
            }
        }

        fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
            panic!("ClosingDispatcher does not implement timers");
        }

        fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
            panic!("ClosingDispatcher does not implement realtime work");
        }
    }

    fn run_runnable(runnable: RunnableVariant) {
        let metadata = runnable.metadata();
        let _timing = crate::profiler::task_timing(metadata.spawned, metadata.location);
        runnable.run();
    }

    fn retain_foreground_activity() {
        let now = Instant::now();
        journal::record_input(journal::InputTiming {
            kind: "test",
            start: now,
            end: now,
            caused_invalidation: false,
        });
    }

    fn has_idle(entries: &[ForegroundJournalEntry]) -> bool {
        entries.iter().any(|entry| {
            matches!(
                entry,
                ForegroundJournalEntry::Boundary(IntervalBoundary::Idle { .. })
            )
        })
    }

    #[test]
    fn refused_send_submission_and_wakeup_settle_only_originating_runnable_counts() {
        for late_wake in [false, true] {
            thread::spawn(move || {
                let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
                let mut collector = journal.collector();
                let main_thread = thread::current().id();
                let (sender, mut receiver) = PriorityQueueReceiver::new();
                let dispatcher = Arc::new(ClosingDispatcher {
                    sender,
                    main_thread,
                    rejected_on: Mutex::new(Vec::new()),
                });
                let foreground = ForegroundExecutor::new(dispatcher.clone());
                assert!(!foreground.uses_test_dispatcher());
                let marker = foreground.spawn(async { 42 });
                let marker_runnable = receiver.try_pop().unwrap().unwrap();
                let send_executor = foreground.to_send();
                drop(foreground);

                let polls = Arc::new(AtomicUsize::new(0));
                let (waker_tx, waker_rx) = std::sync::mpsc::sync_channel(1);
                let submit = {
                    let polls = polls.clone();
                    move || {
                        assert_ne!(thread::current().id(), main_thread);
                        send_executor
                            .spawn(future::poll_fn(move |cx| {
                                assert_eq!(thread::current().id(), main_thread);
                                assert_eq!(polls.fetch_add(1, Ordering::SeqCst), 0);
                                waker_tx.send(cx.waker().clone()).unwrap();
                                Poll::<()>::Pending
                            }))
                            .detach();
                        thread::current().id()
                    }
                };

                let rejected_on = if late_wake {
                    thread::spawn(submit)
                        .join()
                        .expect("worker submission must succeed");
                    retain_foreground_activity();
                    run_runnable(receiver.try_pop().unwrap().unwrap());
                    assert!(!has_idle(&collector.collect_unseen().entries));
                    assert_eq!(polls.load(Ordering::SeqCst), 1);
                    let waker = waker_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    drop(receiver);
                    thread::spawn(move || {
                        waker.wake();
                        thread::current().id()
                    })
                    .join()
                    .expect("worker wake rejection must succeed")
                } else {
                    drop(receiver);
                    thread::spawn(submit)
                        .join()
                        .expect("worker submission rejection must succeed")
                };
                assert_eq!(*dispatcher.rejected_on.lock(), vec![rejected_on]);
                assert_eq!(polls.load(Ordering::SeqCst), usize::from(late_wake));
                {
                    let _turn = journal::foreground_turn();
                    retain_foreground_activity();
                }
                assert!(!has_idle(&collector.collect_unseen().entries));
                run_runnable(marker_runnable);
                assert_eq!(marker.now_or_never(), Some(42));
                assert!(has_idle(&collector.collect_unseen().entries));
            })
            .join()
            .expect("refusal must balance the original foreground counter");
        }
    }
}
