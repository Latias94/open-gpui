//! Ordinary, independently locked consumers of official Zed's sendable foreground mechanism
//! (2c2f737ab2f3db793b7a313bc8e5307f1708d2e8, fixed at 635026fabc38440fce5e1767d125026a17b12012).
//! These execute real tasks through the existing public, doc-hidden PlatformDispatcher bridge.
//! The channel captures that dispatch destination; it is not an App, native loop or renderer.
//! No test-support, additional dependency, profiler-specific branch or signature-only closure
//! is required. Closed native queues and native Submitted receipts have separate consumers.

#![cfg(not(target_family = "wasm"))]

use open_gpui::{
    ForegroundExecutor, PlatformDispatcher, Priority, RunnableVariant, SchedulerLocalExecutor,
    SendForegroundExecutor, profiler,
};
use std::{
    cell::Cell,
    future::{Future, poll_fn},
    pin::{Pin, pin},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    task::{Context, Poll, Wake, Waker},
    thread::{self, ThreadId},
    time::Duration,
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

struct Dispatch {
    submitted_on: ThreadId,
    priority: Priority,
    runnable: RunnableVariant,
}

struct CapturedDispatcher {
    owner: ThreadId,
    sender: Sender<Dispatch>,
}

impl CapturedDispatcher {
    fn new() -> (Arc<Self>, Receiver<Dispatch>) {
        let (sender, receiver) = mpsc::channel();
        (
            Arc::new(Self {
                owner: thread::current().id(),
                sender,
            }),
            receiver,
        )
    }
}

impl PlatformDispatcher for CapturedDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.owner
    }

    fn dispatch(&self, _: RunnableVariant, _: Priority) {
        panic!("this consumer must use foreground dispatch only");
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
        match self.sender.send(Dispatch {
            submitted_on: thread::current().id(),
            priority,
            runnable,
        }) {
            Ok(()) => true,
            Err(error) => {
                if self.is_main_thread() {
                    drop(error.0);
                } else {
                    std::mem::forget(error.0);
                }
                false
            }
        }
    }

    fn dispatch_after(&self, _: Duration, _: RunnableVariant) {
        panic!("this consumer wakes its original task directly, without a timer");
    }

    fn spawn_realtime(&self, _: Box<dyn FnOnce() + Send>) {
        panic!("this consumer must not start realtime work");
    }
}

fn run_next(receiver: &Receiver<Dispatch>, owner: ThreadId) -> ThreadId {
    assert_eq!(thread::current().id(), owner);
    let dispatch = receiver
        .recv_timeout(WAIT_TIMEOUT)
        .expect("the captured foreground destination must receive a runnable");
    assert_eq!(dispatch.priority, Priority::default());
    let metadata = dispatch.runnable.metadata();
    let _timing = profiler::task_timing(metadata.spawned, metadata.location);
    dispatch.runnable.run();
    dispatch.submitted_on
}

fn completed<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the owner must finish the task before its result is consumed"),
    }
}

#[derive(Default)]
struct Gate(Mutex<(bool, Option<Waker>)>);

impl Gate {
    fn poll(&self, cx: &mut Context<'_>) -> Poll<()> {
        let mut state = self.0.lock().expect("the gate must not be poisoned");
        if state.0 {
            Poll::Ready(())
        } else {
            state.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn open(&self) {
        let waker = {
            let mut state = self.0.lock().expect("the gate must not be poisoned");
            state.0 = true;
            state.1.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

struct DropProbe(Sender<ThreadId>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(thread::current().id());
    }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn assert_send_sync_clone<T: Send + Sync + Clone>(_: &T) {}

#[test]
fn worker_submission_and_wakeup_poll_only_the_captured_owner_and_return_cell() {
    let (dispatcher, receiver) = CapturedDispatcher::new();
    let owner = thread::current().id();
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let send: SendForegroundExecutor = foreground.to_send();
    assert_send_sync_clone(&send);
    let clone = send.clone();
    drop(send);
    let gate = Arc::new(Gate::default());
    let polls = Arc::new(AtomicUsize::new(0));
    let (dropped, drops) = mpsc::channel();
    let probe = DropProbe(dropped);
    let worker_gate = gate.clone();
    let observed_polls = polls.clone();
    let (mut task, submitter) = thread::spawn(move || {
        assert_ne!(thread::current().id(), owner);
        let value = Cell::new(40);
        let task = clone.spawn(poll_fn(move |cx| {
            let _capture = &probe;
            assert_eq!(thread::current().id(), owner);
            observed_polls.fetch_add(1, Ordering::SeqCst);
            worker_gate.poll(cx).map(|()| {
                value.set(value.get() + 2);
                Cell::new(value.get())
            })
        }));
        (task, thread::current().id())
    })
    .join()
    .expect("the worker must submit without polling the future");
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert!(!task.is_ready());

    let wakes = Arc::new(WakeCount::default());
    let waiter = Waker::from(wakes.clone());
    assert!(
        Pin::new(&mut task)
            .poll(&mut Context::from_waker(&waiter))
            .is_pending()
    );
    assert_eq!(run_next(&receiver, owner), submitter);
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!task.is_ready());
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Empty)));

    let waking_thread = thread::spawn(move || {
        gate.open();
        thread::current().id()
    })
    .join()
    .expect("a worker must wake the already-polled task");
    assert_ne!(waking_thread, owner);
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(run_next(&receiver, owner), waking_thread);
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert!(task.is_ready());
    assert!(wakes.0.load(Ordering::SeqCst) > 0);
    assert_eq!(
        drops.try_recv().expect("completion must drop the future"),
        owner
    );
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));
    let value = thread::spawn(move || {
        let output: Cell<usize> = completed(task);
        output.set(output.get() + 1);
        output.into_inner()
    })
    .join()
    .expect("the Send task and its non-Sync result must cross back to a worker");
    assert_eq!(value, 43);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn facade_and_scheduler_handles_preserve_two_distinct_dispatch_destinations() {
    let owner = thread::current().id();
    let (first_dispatcher, first_queue) = CapturedDispatcher::new();
    let (second_dispatcher, second_queue) = CapturedDispatcher::new();
    let first = ForegroundExecutor::new(first_dispatcher).to_send();
    let second = ForegroundExecutor::new(second_dispatcher)
        .scheduler_executor()
        .to_send();
    assert_send_sync_clone(&second);
    let (first_task, second_task) = thread::spawn(move || {
        let first_task = first.spawn(async move {
            assert_eq!(thread::current().id(), owner);
            Cell::new(11)
        });
        let second_task = second.clone().spawn(async move {
            assert_eq!(thread::current().id(), owner);
            Cell::new(22)
        });
        (first_task, second_task)
    })
    .join()
    .expect("both handles must be independently usable from the worker");
    assert!(!first_task.is_ready());
    assert!(!second_task.is_ready());
    run_next(&first_queue, owner);
    assert!(first_task.is_ready());
    assert!(!second_task.is_ready());
    assert_eq!(completed(first_task).into_inner(), 11);
    run_next(&second_queue, owner);
    assert_eq!(completed(second_task).into_inner(), 22);
    assert!(matches!(
        first_queue.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    assert!(matches!(
        second_queue.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
}

#[test]
fn send_handle_retains_the_scheduler_and_destination_after_the_local_executor_is_dropped() {
    let (dispatcher, receiver) = CapturedDispatcher::new();
    let destination = Arc::downgrade(&dispatcher);
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let source = foreground.scheduler_executor();
    let scheduler = Arc::downgrade(source.scheduler());
    let local = SchedulerLocalExecutor::new(
        source.session_id(),
        source.scheduler().clone(),
        move |runnable| {
            assert!(dispatcher.dispatch_on_main_thread(runnable, Priority::default()));
        },
    );
    let send = local.to_send();
    let clone = send.clone();
    drop((source, local, foreground, send));
    assert!(scheduler.upgrade().is_some());
    assert!(destination.upgrade().is_some());
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    thread::spawn(move || drop(clone))
        .join()
        .expect("the final Send handle may be dropped on a worker");
    assert!(scheduler.upgrade().is_none());
    assert!(destination.upgrade().is_none());
    assert!(matches!(
        receiver.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
}

#[test]
fn dropping_a_queued_task_on_the_worker_cancels_before_its_first_poll() {
    let (dispatcher, receiver) = CapturedDispatcher::new();
    let foreground = ForegroundExecutor::new(dispatcher);
    let send = foreground.to_send();
    let owner = thread::current().id();
    let polls = Arc::new(AtomicUsize::new(0));
    let observed_polls = polls.clone();
    let (dropped, drops) = mpsc::channel();
    let probe = DropProbe(dropped);
    thread::spawn(move || {
        let task = send.spawn(async move {
            let _capture = probe;
            observed_polls.fetch_add(1, Ordering::SeqCst);
            Cell::new(1)
        });
        drop(task);
    })
    .join()
    .expect("the submitting worker must cancel its queued task");
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Empty)));
    run_next(&receiver, owner);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(
        drops
            .try_recv()
            .expect("the cancelled capture must be released"),
        owner
    );
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn dropping_a_suspended_task_releases_captures_and_a_late_wake_does_not_repoll() {
    let (dispatcher, receiver) = CapturedDispatcher::new();
    let foreground = ForegroundExecutor::new(dispatcher);
    let send = foreground.to_send();
    let owner = thread::current().id();
    let gate = Arc::new(Gate::default());
    let task_gate = gate.clone();
    let polls = Arc::new(AtomicUsize::new(0));
    let observed_polls = polls.clone();
    let (dropped, drops) = mpsc::channel();
    let probe = DropProbe(dropped);
    let task = thread::spawn(move || {
        send.spawn(poll_fn(move |cx| {
            let _capture = &probe;
            assert_eq!(thread::current().id(), owner);
            observed_polls.fetch_add(1, Ordering::SeqCst);
            task_gate.poll(cx).map(|()| Cell::new(1))
        }))
    })
    .join()
    .expect("the worker must return its suspended task handle");
    run_next(&receiver, owner);
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!task.is_ready());
    thread::spawn(move || drop(task))
        .join()
        .expect("the worker must cancel its suspended task");
    run_next(&receiver, owner);
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(
        drops
            .try_recv()
            .expect("owner cleanup must drop the capture"),
        owner
    );
    thread::spawn(move || gate.open())
        .join()
        .expect("the saved waker must remain safe after cancellation");
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));
}

#[test]
fn detached_work_outlives_all_executor_handles_then_releases_the_dispatch_destination() {
    let (dispatcher, receiver) = CapturedDispatcher::new();
    let destination = Arc::downgrade(&dispatcher);
    let foreground = ForegroundExecutor::new(dispatcher);
    let local = foreground.scheduler_executor();
    let scheduler = Arc::downgrade(local.scheduler());
    let send = foreground.to_send();
    let owner = thread::current().id();
    let gate = Arc::new(Gate::default());
    let task_gate = gate.clone();
    let (dropped, drops) = mpsc::channel();
    let probe = DropProbe(dropped);
    drop((local, foreground));
    thread::spawn(move || {
        send.spawn(poll_fn(move |cx| {
            let _capture = &probe;
            assert_eq!(thread::current().id(), owner);
            task_gate.poll(cx)
        }))
        .detach();
    })
    .join()
    .expect("detachment must consume the task without cancelling it");
    assert!(scheduler.upgrade().is_some());
    assert!(destination.upgrade().is_some());
    run_next(&receiver, owner);
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Empty)));
    thread::spawn(move || gate.open())
        .join()
        .expect("a worker must resume detached work after the handles were dropped");
    run_next(&receiver, owner);
    assert_eq!(
        drops
            .try_recv()
            .expect("detached completion must release its capture"),
        owner
    );
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));
    assert!(scheduler.upgrade().is_none());
    assert!(destination.upgrade().is_none());
    assert!(matches!(
        receiver.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
}
