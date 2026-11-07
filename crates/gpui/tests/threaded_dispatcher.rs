#![cfg(feature = "test-support")]

//! Public runtime coverage for Zed's threaded-dispatcher prerequisite at
//! 635026fabc38440fce5e1767d125026a17b12012, adapted to the fork's admission result.
//!
//! The external thread models an untracked callback. Its submission is acknowledged
//! before another main-queue turn; quiescence alone does not await external callbacks.
//! No OS-native event loop, App/Window workflow, or benchmark context is exercised.
//! Each test creates one dispatcher, following upstream's process-lifetime contract.
//! State release is checked while the dispatcher is alive, without assuming that
//! dropping it stops its worker or timer threads.

use futures::{FutureExt as _, channel::oneshot};
use open_gpui::{
    BackgroundExecutor, ForegroundExecutor, PlatformDispatcher as _, Priority, RunnableMeta,
    ThreadedDispatcher,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, mpsc},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const TIMER_DELAY: Duration = Duration::from_millis(20);
const DISTANT_DEADLINE: Duration = Duration::from_secs(60 * 60);

struct LocalState {
    created_on: ThreadId,
    value: Cell<usize>,
    dropped_on: Rc<Cell<Option<ThreadId>>>,
}

impl LocalState {
    fn new(dropped_on: Rc<Cell<Option<ThreadId>>>) -> Self {
        Self {
            created_on: thread::current().id(),
            value: Cell::new(0),
            dropped_on,
        }
    }

    fn add(&self, amount: usize) {
        assert_eq!(thread::current().id(), self.created_on);
        self.value.set(self.value.get() + amount);
    }
}

impl Drop for LocalState {
    fn drop(&mut self) {
        self.dropped_on.set(Some(thread::current().id()));
    }
}

/// Executes background, idle, and external-callback work through public interfaces.
/// The completed foreground Task owns its Rc result until explicitly released.
#[test]
fn public_executors_and_external_main_admission_release_local_state() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let main_thread = thread::current().id();
    assert!(dispatcher.is_main_thread());
    for executor_dispatcher in [background.dispatcher(), foreground.dispatcher()] {
        assert!(std::ptr::eq(
            executor_dispatcher
                .as_threaded()
                .expect("the public executors must use the threaded dispatcher"),
            dispatcher.as_ref(),
        ));
    }

    let dropped_on = Rc::new(Cell::new(None));
    let state = Rc::new(LocalState::new(dropped_on.clone()));
    let weak = Rc::downgrade(&state);
    let idle_state = state.clone();
    let (release_background, background_gate) = oneshot::channel();
    let (handoff_sender, handoff_receiver) = oneshot::channel();
    let (callback_sender, callback_receiver) = oneshot::channel();

    let background_task = background.spawn_with_priority(Priority::Low, async move {
        background_gate
            .await
            .expect("the test must release the background handoff");
        let worker_thread = thread::current().id();
        assert_ne!(worker_thread, main_thread);
        handoff_sender
            .send((worker_thread, 40))
            .expect("the foreground handoff receiver must remain alive");
        worker_thread
    });
    let local_task = foreground.spawn(async move {
        state.add(1);
        let (worker_thread, value) = handoff_receiver
            .await
            .expect("the background task must deliver its handoff");
        assert_ne!(worker_thread, main_thread);
        state.add(value);
        let callback_thread = callback_receiver
            .await
            .expect("the external callback must resume the original foreground task");
        assert_ne!(callback_thread, main_thread);
        state.add(1);
        state
    });
    let idle_task = foreground.spawn_when_idle(None, async move {
        idle_state.add(1);
    });

    assert!(dispatcher.run_ready_main_tasks());
    assert!(idle_task.is_ready());
    assert!(!local_task.is_ready());
    assert_eq!(weak.upgrade().expect("state must be live").value.get(), 2);
    assert!(!dispatcher.run_ready_main_tasks());

    release_background
        .send(())
        .expect("the background task must still be waiting");
    let Ok(worker_thread) = foreground.block_with_timeout(WAIT_TIMEOUT, background_task) else {
        panic!("background handoff timed out: {}", dispatcher.debug_state());
    };
    assert_ne!(worker_thread, main_thread);
    dispatcher.run_until_idle();
    assert!(!local_task.is_ready());
    assert_eq!(weak.upgrade().expect("state must be live").value.get(), 42);
    assert!(!dispatcher.run_ready_main_tasks());

    let reschedule_dispatcher = dispatcher.clone();
    let (runnable, admitted_task) = async_task::Builder::new()
        .metadata(RunnableMeta::new_with_callers_location())
        .spawn(
            |_| async { thread::current().id() },
            move |runnable| {
                assert!(reschedule_dispatcher.dispatch_on_main_thread(runnable, Priority::Low));
            },
        );
    let callback_dispatcher = dispatcher.clone();
    let (admission_sender, admission_receiver) = mpsc::channel();
    let callback_thread = thread::spawn(move || {
        let accepted = callback_dispatcher.dispatch_on_main_thread(runnable, Priority::Low);
        callback_sender
            .send(thread::current().id())
            .expect("the original foreground callback receiver must remain alive");
        admission_sender
            .send(accepted)
            .expect("the test must receive the main-thread admission result");
    });
    assert!(
        admission_receiver
            .recv_timeout(WAIT_TIMEOUT)
            .expect("the external callback must acknowledge its submission")
    );
    callback_thread
        .join()
        .expect("the acknowledged callback must finish");

    assert!(dispatcher.run_ready_main_tasks());
    dispatcher.run_until_idle();
    assert_eq!(admitted_task.now_or_never(), Some(main_thread));
    assert!(local_task.is_ready());
    assert_eq!(
        weak.upgrade()
            .expect("Task must own its result")
            .value
            .get(),
        43
    );
    assert_eq!(dropped_on.get(), None);

    drop((local_task, idle_task));
    dispatcher.run_until_idle();
    assert!(!dispatcher.run_ready_main_tasks());
    assert!(
        weak.upgrade().is_none(),
        "settled queues must release Rc state"
    );
    assert_eq!(dropped_on.get(), Some(main_thread));
}

/// Uses actual timer completion and cancellation to resume the original waiter.
/// Dropping a suspended local Task must also release Rc state before dispatcher drop.
#[test]
fn real_timers_and_cancellation_release_original_waiters_and_local_state() {
    let dispatcher = Arc::new(ThreadedDispatcher::new());
    let background = BackgroundExecutor::new(dispatcher.clone());
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let main_thread = thread::current().id();
    let dropped_on = Rc::new(Cell::new(None));
    let state = Rc::new(LocalState::new(dropped_on.clone()));
    let weak = Rc::downgrade(&state);

    let started = Instant::now();
    let short_timer = background.timer(TIMER_DELAY);
    let distant_timer = background.timer(DISTANT_DEADLINE);
    let (timer_sender, timer_receiver) = mpsc::channel();
    let original_waiter = background.spawn(async move {
        short_timer.await;
        timer_sender
            .send(Instant::now())
            .expect("the test must observe real timer completion");
        distant_timer.await;
        timer_sender
            .send(Instant::now())
            .expect("the test must observe the original waiter's cancellation wake");
        40
    });
    let local_task = foreground.spawn(async move {
        state.add(1);
        state.add(original_waiter.await);
        state
    });

    assert!(dispatcher.run_ready_main_tasks());
    assert!(!local_task.is_ready());
    let fired = timer_receiver
        .recv_timeout(WAIT_TIMEOUT)
        .expect("the real timer must wake its original background waiter");
    assert!(fired.duration_since(started) >= TIMER_DELAY);
    dispatcher.run_until_idle();
    assert!(!local_task.is_ready());
    assert_eq!(weak.upgrade().expect("state must be live").value.get(), 1);
    assert!(!dispatcher.run_ready_main_tasks());

    let canceled_at = Instant::now();
    assert_eq!(dispatcher.cancel_pending_timers(), 1);
    let resumed = timer_receiver
        .recv_timeout(WAIT_TIMEOUT)
        .expect("canceling the timer must wake the original waiter promptly");
    assert!(resumed >= canceled_at);
    assert!(resumed < started + DISTANT_DEADLINE);
    dispatcher.run_until_idle();
    assert!(local_task.is_ready());
    assert_eq!(
        weak.upgrade()
            .expect("Task must own its result")
            .value
            .get(),
        41
    );
    assert_eq!(dropped_on.get(), None);
    drop(local_task);
    dispatcher.run_until_idle();
    assert!(
        weak.upgrade().is_none(),
        "timer settlement must release Rc state"
    );
    assert_eq!(dropped_on.get(), Some(main_thread));
    assert_eq!(dispatcher.cancel_pending_timers(), 0);

    let dropped_on = Rc::new(Cell::new(None));
    let state = Rc::new(LocalState::new(dropped_on.clone()));
    let weak = Rc::downgrade(&state);
    let timer = background.timer(DISTANT_DEADLINE);
    let canceled_task = foreground.spawn_when_idle(None, async move {
        state.add(1);
        timer.await;
        state.add(100);
        state
    });
    assert!(dispatcher.run_ready_main_tasks());
    dispatcher.run_until_idle();
    assert!(!canceled_task.is_ready());
    assert_eq!(
        weak.upgrade()
            .expect("suspended state must be live")
            .value
            .get(),
        1
    );

    drop(canceled_task);
    dispatcher.run_until_idle();
    assert!(
        weak.upgrade().is_none(),
        "task cancellation must release Rc state"
    );
    assert_eq!(dropped_on.get(), Some(main_thread));
    assert_eq!(dispatcher.cancel_pending_timers(), 1);
    dispatcher.run_until_idle();
    assert_eq!(dispatcher.cancel_pending_timers(), 0);
    assert!(!dispatcher.run_ready_main_tasks());
    assert!(
        weak.upgrade().is_none(),
        "timer cleanup must not retain canceled state"
    );
}
