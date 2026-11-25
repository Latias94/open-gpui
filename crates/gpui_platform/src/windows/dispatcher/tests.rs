use std::{
    future::{Future as _, poll_fn},
    rc::Rc,
    sync::{Arc, mpsc},
    task::Poll,
};

use futures::channel::oneshot;
use open_gpui::{BackgroundExecutor, ForegroundExecutor, PriorityQueueReceiver};
use open_gpui_scheduler::{LocalExecutor, SendLocalExecutor};

use super::*;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

fn test_dispatcher() -> (
    Arc<WindowsDispatcher>,
    PriorityQueueReceiver<RunnableVariant>,
) {
    let (sender, receiver) = PriorityQueueReceiver::new();
    (
        Arc::new(WindowsDispatcher::new(sender, HWND::default(), 0)),
        receiver,
    )
}

fn capture_send_runnables(
    foreground: &LocalExecutor,
) -> (SendLocalExecutor, mpsc::Receiver<RunnableVariant>) {
    let (queued, runnables) = mpsc::channel();
    let executor = LocalExecutor::new(
        foreground.session_id(),
        foreground.scheduler().clone(),
        move |runnable| {
            queued
                .send(runnable)
                .expect("test owns the runnable receiver")
        },
    );
    (executor.to_send(), runnables)
}

struct ReportDropThread(mpsc::Sender<ThreadId>);

impl Drop for ReportDropThread {
    fn drop(&mut self) {
        let _ = self.0.send(current().id());
    }
}

#[test]
fn win32_workers_repoll_and_release_background_tasks() {
    let (dispatcher, _receiver) = test_dispatcher();
    let foreground = ForegroundExecutor::new(dispatcher.clone());
    let background = BackgroundExecutor::new(dispatcher);
    let main_thread = current().id();

    for priority in [Priority::High, Priority::Medium, Priority::Low] {
        let (report_drop, dropped) = mpsc::channel();
        let drop_probe = ReportDropThread(report_drop);
        let mut first_poll = None;
        let task = background.spawn_with_priority(
            priority,
            poll_fn(move |cx| {
                let _keep_alive = &drop_probe;
                let worker = current().id();
                if let Some(first_worker) = first_poll.replace(worker) {
                    Poll::Ready([first_worker, worker])
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }),
        );
        let Ok([first_worker, second_worker]) =
            foreground.block_with_timeout(CALLBACK_TIMEOUT, task)
        else {
            panic!("Win32 worker did not complete {priority:?} work after a self-wake");
        };
        assert_ne!(first_worker, main_thread);
        assert_ne!(second_worker, main_thread);
        assert_eq!(dropped.recv_timeout(CALLBACK_TIMEOUT), Ok(second_worker));
        assert_eq!(dropped.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }
}

#[test]
fn win32_timer_wakes_local_task_after_foreground_queue_closes() {
    let (dispatcher, mut main_receiver) = test_dispatcher();
    let foreground = ForegroundExecutor::new(dispatcher.clone()).scheduler_executor();
    let (timer_executor, timer_runnables) = capture_send_runnables(&foreground);
    let (wake_local, local_wait) = oneshot::channel();
    let (record_admission, admissions) = mpsc::channel();
    let local_capture = Rc::new(());
    let local_lifetime = Rc::downgrade(&local_capture);
    let local_dispatcher = dispatcher.clone();
    let main_thread = current().id();

    dispatcher.wake_posted.store(true, Ordering::Release);
    let local_task = foreground.spawn_with_dispatch(
        async move {
            let _keep_alive = local_capture;
            local_wait.await.is_ok()
        },
        move |runnable| {
            let accepted = local_dispatcher.dispatch_on_main_thread(runnable, Priority::Medium);
            let _ = record_admission.send((current().id(), accepted));
        },
    );
    assert_eq!(admissions.try_recv(), Ok((main_thread, true)));
    let initial = main_receiver
        .try_pop()
        .unwrap()
        .expect("initial local poll");
    WindowsDispatcher::execute_runnable(initial);
    assert!(main_receiver.is_empty());
    drop(main_receiver);
    dispatcher.wake_posted.store(false, Ordering::Release);

    let timer_task = timer_executor.spawn(async move { (current().id(), wake_local.send(())) });
    dispatcher.dispatch_after(
        Duration::from_millis(20),
        timer_runnables
            .try_recv()
            .expect("live Send timer runnable"),
    );

    let Ok((timer_thread, Ok(()))) = foreground.block_with_timeout(CALLBACK_TIMEOUT, timer_task)
    else {
        panic!("Win32 timer did not signal the waiting local task");
    };
    assert_ne!(timer_thread, main_thread);
    assert_eq!(admissions.try_recv(), Ok((timer_thread, false)));
    assert_eq!(admissions.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert!(!dispatcher.wake_posted.load(Ordering::Acquire));
    assert!(!local_task.is_ready());
    assert!(local_lifetime.upgrade().is_some());
}

#[test]
fn win32_timer_releases_cancelled_send_task_without_polling() {
    let (dispatcher, receiver) = test_dispatcher();
    let foreground = ForegroundExecutor::new(dispatcher.clone()).scheduler_executor();
    let (timer_executor, timer_runnables) = capture_send_runnables(&foreground);
    let (events, observed) = mpsc::channel();
    let drop_probe = ReportDropThread(events.clone());
    let task = timer_executor.spawn(async move {
        let _keep_alive = drop_probe;
        let _ = events.send(current().id());
    });
    let runnable = timer_runnables
        .try_recv()
        .expect("cancelled timer runnable");
    drop(task);
    assert_eq!(observed.try_recv(), Err(mpsc::TryRecvError::Empty));
    drop(timer_executor);
    drop(foreground);
    drop(receiver);

    let dispatcher_lifetime = Arc::downgrade(&dispatcher);
    dispatcher.dispatch_after(Duration::from_nanos(1), runnable);
    drop(dispatcher);
    assert!(dispatcher_lifetime.upgrade().is_none());
    let dropped_on = observed
        .recv_timeout(CALLBACK_TIMEOUT)
        .expect("timer must drop capture");
    assert_ne!(dropped_on, current().id());
    assert_eq!(
        observed.recv_timeout(CALLBACK_TIMEOUT),
        Err(mpsc::RecvTimeoutError::Disconnected),
    );
}

#[test]
fn win32_timer_due_time_rounds_up_and_saturates_as_a_relative_filetime() {
    for (duration, expected_ticks) in [
        (Duration::ZERO, -1),
        (Duration::from_nanos(1), -1),
        (Duration::from_nanos(100), -1),
        (Duration::from_nanos(101), -2),
        (Duration::from_millis(20), -200_000),
        (Duration::MAX, -i64::MAX),
    ] {
        let due = relative_timer_due_time(duration);
        let ticks = ((u64::from(due.dwHighDateTime) << 32) | u64::from(due.dwLowDateTime)) as i64;
        assert_eq!(ticks, expected_ticks, "{duration:?}");
    }
}

#[test]
fn win32_pool_rejection_does_not_fire_the_timer_or_cancel_the_task() {
    let (dispatcher, _receiver) = test_dispatcher();
    let foreground = ForegroundExecutor::new(dispatcher).scheduler_executor();
    let (executor, queued) = capture_send_runnables(&foreground);
    let (complete, waiting) = oneshot::channel();
    let task = executor.spawn(async move {
        let _ = complete.send(());
    });
    let mut timer = std::pin::pin!(open_gpui_scheduler::Timer::new(waiting));
    let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
    let runnable = queued
        .try_recv()
        .expect("the timer has a scheduled runnable");
    let mut rejected = None;
    let result = submit_pool_runnable(runnable, |context| {
        rejected = NonNull::new(context.cast());
        Err(windows::Win32::Foundation::E_FAIL.into())
    });
    let pending = timer.as_mut().poll(&mut cx).is_pending();
    let task_ready = task.is_ready();
    drop(task);
    // SAFETY: The injected native rejection retained this raw reference and has no callback.
    drop(unsafe { RunnableVariant::from_raw(rejected.expect("the refused context is retained")) });

    assert!(result.is_err());
    assert!(
        pending,
        "a native rejection cannot be observed as a fired timer"
    );
    assert!(
        !task_ready,
        "the scheduler did not cancel the rejected task"
    );
}
