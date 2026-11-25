use calloop::{
    EventLoop, PostAction,
    channel::{self, Sender},
    timer::TimeoutAction,
};
use open_gpui_util::ResultExt;

use std::{
    mem::MaybeUninit,
    sync::{Arc, Mutex, PoisonError},
    thread,
    time::Duration,
};

use open_gpui::{
    PlatformDispatcher, Priority, PriorityQueueReceiver, PriorityQueueSender, RunnableVariant,
    profiler,
};

struct TimerAfter {
    duration: Duration,
    runnable: RunnableVariant,
}

enum TimerMessage {
    After(TimerAfter),
    Shutdown,
}

pub(crate) struct LinuxDispatcher {
    main_sender: PriorityQueueCalloopSender<RunnableVariant>,
    timer_sender: Sender<TimerMessage>,
    background_sender: PriorityQueueSender<RunnableVariant>,
    _background_threads: Vec<thread::JoinHandle<()>>,
    main_thread_id: thread::ThreadId,
}

const MIN_THREADS: usize = 2;

impl LinuxDispatcher {
    pub fn new(main_sender: PriorityQueueCalloopSender<RunnableVariant>) -> Self {
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        let thread_count =
            std::thread::available_parallelism().map_or(MIN_THREADS, |i| i.get().max(MIN_THREADS));

        let mut background_threads = (0..thread_count)
            .map(|i| {
                let receiver: PriorityQueueReceiver<RunnableVariant> = background_receiver.clone();
                std::thread::Builder::new()
                    .name(format!("Worker-{i}"))
                    .spawn(move || {
                        for runnable in receiver.iter() {
                            let location = runnable.metadata().location;
                            let spawned = runnable.metadata().spawned;
                            let _task_timing = profiler::task_timing(spawned, location);
                            runnable.run();
                        }
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let (timer_sender, timer_channel) = calloop::channel::channel::<TimerMessage>();
        let timer_thread = std::thread::Builder::new()
            .name("Timer".to_owned())
            .spawn(move || {
                let mut event_loop: EventLoop<bool> =
                    EventLoop::try_new().expect("Failed to initialize timer loop!");

                let handle = event_loop.handle();
                let timer_handle = handle.downgrade();
                let signal = event_loop.get_signal();
                handle
                    .insert_source(timer_channel, move |event, _, stopped| match event {
                        channel::Event::Msg(TimerMessage::Shutdown) | channel::Event::Closed => {
                            *stopped = true;
                            signal.stop();
                        }
                        channel::Event::Msg(TimerMessage::After(timer)) => {
                            if !*stopped {
                                let mut runnable = Some(timer.runnable);
                                timer_handle
                                    .upgrade()
                                    .expect("timer loop must own its handle while dispatching")
                                    .insert_source(
                                        calloop::timer::Timer::from_duration(timer.duration),
                                        move |_, _, stopped| {
                                            if !*stopped && let Some(runnable) = runnable.take() {
                                                let location = runnable.metadata().location;
                                                let spawned = runnable.metadata().spawned;
                                                let _task_timing =
                                                    profiler::task_timing(spawned, location);
                                                runnable.run();
                                            }
                                            TimeoutAction::Drop
                                        },
                                    )
                                    .expect("Failed to start timer");
                            }
                        }
                    })
                    .expect("Failed to start timer thread");

                event_loop.run(None, &mut false, |_| {}).log_err();
            })
            .unwrap();

        background_threads.push(timer_thread);

        Self {
            main_sender,
            timer_sender,
            background_sender,
            _background_threads: background_threads,
            main_thread_id: thread::current().id(),
        }
    }

    pub(crate) fn shutdown_timers(&self) {
        self.timer_sender.send(TimerMessage::Shutdown).ok();
    }
}

impl Drop for LinuxDispatcher {
    fn drop(&mut self) {
        self.shutdown_timers();
    }
}

impl PlatformDispatcher for LinuxDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.background_sender
            .send(priority, runnable)
            .unwrap_or_else(|_| panic!("blocking sender returned without value"));
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
        match self.main_sender.send(priority, runnable) {
            Ok(()) => true,
            Err(runnable) => {
                if self.is_main_thread() {
                    drop(runnable);
                    return false;
                }
                // NOTE: Runnable may wrap a Future that is !Send.
                //
                // This is usually safe because we only poll it on the main thread.
                // However if the send fails, we know that:
                // 1. main_receiver has been dropped (which implies the app is shutting down)
                // 2. we are on a background thread.
                // It is not safe to drop something !Send on the wrong thread, and
                // the app will exit soon anyway, so we must forget the runnable.
                std::mem::forget(runnable);
                false
            }
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.timer_sender
            .send(TimerMessage::After(TimerAfter { duration, runnable }))
            .ok();
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            // SAFETY: always safe to call
            let thread_id = unsafe { libc::pthread_self() };

            let policy = libc::SCHED_FIFO;
            let sched_priority = 65;

            // SAFETY: all sched_param members are valid when initialized to zero.
            let mut sched_param =
                unsafe { MaybeUninit::<libc::sched_param>::zeroed().assume_init() };
            sched_param.sched_priority = sched_priority;
            // SAFETY: sched_param is a valid initialized structure
            let result = unsafe { libc::pthread_setschedparam(thread_id, policy, &sched_param) };
            if result != 0 {
                log::warn!("failed to set realtime thread priority");
            }

            f();
        });
    }
}

pub struct PriorityQueueCalloopSender<T> {
    sender: Arc<Mutex<Option<PriorityQueueSender<T>>>>,
    ping: calloop::ping::Ping,
}

impl<T> PriorityQueueCalloopSender<T> {
    fn send(&self, priority: Priority, item: T) -> Result<(), open_gpui::queue::SendError<T>> {
        let res = match &*self.sender.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(sender) => sender.send(priority, item),
            None => Err(open_gpui::queue::SendError(item)),
        };
        if res.is_ok() {
            self.ping.ping();
        }
        res
    }
}

impl<T> Drop for PriorityQueueCalloopSender<T> {
    fn drop(&mut self) {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(sender);
        self.ping.ping();
    }
}

pub struct PriorityQueueCalloopReceiver<T> {
    receiver: PriorityQueueReceiver<T>,
    sender: Arc<Mutex<Option<PriorityQueueSender<T>>>>,
    source: calloop::ping::PingSource,
    ping: calloop::ping::Ping,
}

impl<T> PriorityQueueCalloopReceiver<T> {
    pub fn new() -> (PriorityQueueCalloopSender<T>, Self) {
        let (ping, source) = calloop::ping::make_ping().expect("Failed to create a Ping.");

        let (tx, rx) = PriorityQueueReceiver::new();
        let sender = Arc::new(Mutex::new(Some(tx)));

        (
            PriorityQueueCalloopSender {
                sender: sender.clone(),
                ping: ping.clone(),
            },
            Self {
                receiver: rx,
                sender,
                source,
                ping,
            },
        )
    }

    pub(crate) fn close(&mut self) -> Vec<T> {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(sender);
        self.receiver
            .clone()
            .try_iter()
            .filter_map(Result::ok)
            .collect()
    }
}

impl<T> Drop for PriorityQueueCalloopReceiver<T> {
    fn drop(&mut self) {
        drop(self.close());
    }
}

use calloop::channel::Event;

#[derive(Debug)]
pub struct ChannelError(calloop::ping::PingError);

impl std::fmt::Display for ChannelError {
    #[cfg_attr(nightly_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for ChannelError {
    #[cfg_attr(nightly_coverage, coverage(off))]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl<T> calloop::EventSource for PriorityQueueCalloopReceiver<T> {
    type Event = Event<T>;
    type Metadata = ();
    type Ret = ();
    type Error = ChannelError;

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        let mut clear_readiness = false;
        let mut disconnected = false;

        let action = self
            .source
            .process_events(readiness, token, |(), &mut ()| {
                let mut is_empty = true;

                let receiver = self.receiver.clone();
                for runnable in receiver.try_iter().take(1024) {
                    match runnable {
                        Ok(r) => {
                            callback(Event::Msg(r), &mut ());
                            is_empty = false;
                        }
                        Err(_) => {
                            disconnected = true;
                        }
                    }
                }

                if disconnected {
                    callback(Event::Closed, &mut ());
                }

                if is_empty {
                    clear_readiness = true;
                }
            })
            .map_err(ChannelError)?;

        if disconnected {
            Ok(PostAction::Remove)
        } else if clear_readiness {
            Ok(action)
        } else {
            // Re-notify the ping source so we can try again.
            self.ping.ping();
            Ok(PostAction::Continue)
        }
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.source.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.source.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.source.unregister(poll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_foreground_receiver_is_idempotent_and_rejects_its_live_sender() {
        let (sender, mut receiver) = PriorityQueueCalloopReceiver::new();
        for value in 0..3 {
            sender.send(Priority::Medium, value).unwrap();
        }
        assert_eq!(receiver.close(), vec![0, 1, 2]);
        assert_eq!(sender.send(Priority::High, 3).unwrap_err().0, 3);
        assert!(receiver.close().is_empty());
        drop(receiver);
        assert_eq!(sender.send(Priority::Low, 4).unwrap_err().0, 4);
    }

    #[test]
    fn calloop_works() {
        let mut event_loop = calloop::EventLoop::try_new().unwrap();
        let handle = event_loop.handle();

        let (tx, rx) = PriorityQueueCalloopReceiver::new();

        struct Data {
            got_msg: bool,
            got_closed: bool,
        }

        let mut data = Data {
            got_msg: false,
            got_closed: false,
        };

        let _channel_token = handle
            .insert_source(rx, move |evt, &mut (), data: &mut Data| match evt {
                Event::Msg(()) => {
                    data.got_msg = true;
                }

                Event::Closed => {
                    data.got_closed = true;
                }
            })
            .unwrap();

        // nothing is sent, nothing is received
        event_loop
            .dispatch(Some(::std::time::Duration::ZERO), &mut data)
            .unwrap();

        assert!(!data.got_msg);
        assert!(!data.got_closed);
        // a message is send

        tx.send(Priority::Medium, ()).unwrap();
        event_loop
            .dispatch(Some(::std::time::Duration::ZERO), &mut data)
            .unwrap();

        assert!(data.got_msg);
        assert!(!data.got_closed);

        // the sender is dropped
        drop(tx);
        event_loop
            .dispatch(Some(::std::time::Duration::ZERO), &mut data)
            .unwrap();

        assert!(data.got_msg);
        assert!(data.got_closed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dispatcher_shutdown_stops_threads_and_drops_pending_timers_with_a_live_channel_sender() {
        use std::sync::{Arc, mpsc};

        struct CaptureDispatcher(mpsc::Sender<RunnableVariant>);

        impl PlatformDispatcher for CaptureDispatcher {
            fn is_main_thread(&self) -> bool {
                false
            }

            fn dispatch(&self, runnable: RunnableVariant, _priority: Priority) {
                self.0.send(runnable).unwrap();
            }

            fn dispatch_on_main_thread(
                &self,
                _runnable: RunnableVariant,
                _priority: Priority,
            ) -> bool {
                unreachable!()
            }

            fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
                unreachable!()
            }

            fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
                unreachable!()
            }
        }

        let timeout = Duration::from_secs(5);
        let (main_sender, _main_receiver) = PriorityQueueCalloopReceiver::new();
        let mut dispatcher = LinuxDispatcher::new(main_sender);
        let retained_timer_sender = dispatcher.timer_sender.clone();
        let (runnable_sender, runnable_receiver) = mpsc::channel();
        let executor =
            open_gpui::BackgroundExecutor::new(Arc::new(CaptureDispatcher(runnable_sender)));

        let (pending_sender, pending_receiver) = mpsc::channel();
        executor
            .spawn(async move {
                pending_sender.send(()).ok();
            })
            .detach();
        dispatcher.dispatch_after(
            Duration::from_secs(3600),
            runnable_receiver.recv_timeout(timeout).unwrap(),
        );

        let (ready_sender, ready_receiver) = mpsc::channel();
        executor
            .spawn(async move {
                ready_sender.send(()).ok();
            })
            .detach();
        dispatcher.dispatch_after(
            Duration::ZERO,
            runnable_receiver.recv_timeout(timeout).unwrap(),
        );
        ready_receiver
            .recv_timeout(timeout)
            .expect("the timer thread should process queued timers");
        assert_eq!(pending_receiver.try_recv(), Err(mpsc::TryRecvError::Empty));

        let threads = std::mem::take(&mut dispatcher._background_threads);
        drop(dispatcher);

        assert_eq!(
            pending_receiver.recv_timeout(timeout),
            Err(mpsc::RecvTimeoutError::Disconnected),
            "shutdown should drop the pending timer's capture without running it"
        );

        let deadline = std::time::Instant::now() + timeout;
        while threads.iter().any(|thread| !thread.is_finished()) {
            assert!(
                std::time::Instant::now() < deadline,
                "explicit shutdown must stop the timer thread while a sender remains alive"
            );
            thread::sleep(Duration::from_millis(1));
        }
        for thread in threads {
            thread
                .join()
                .expect("dispatcher thread should exit cleanly");
        }
        assert!(retained_timer_sender.send(TimerMessage::Shutdown).is_err());
    }
}
