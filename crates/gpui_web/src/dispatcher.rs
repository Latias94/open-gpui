use open_gpui::{
    PlatformDispatcher, Priority, PriorityQueueReceiver, PriorityQueueSender, RunnableVariant,
};
use open_gpui_core_util::defer;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use web_time::Instant;

#[cfg(feature = "multithreaded")]
const MIN_BACKGROUND_THREADS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebDispatcherMode {
    SingleThreaded {
        reason: WebDispatcherSingleThreadedReason,
    },
    Multithreaded {
        background_workers: usize,
    },
}

impl WebDispatcherMode {
    pub fn supports_background_workers(self) -> bool {
        matches!(self, Self::Multithreaded { .. })
    }

    pub fn single_threaded_reason(self) -> Option<WebDispatcherSingleThreadedReason> {
        match self {
            Self::SingleThreaded { reason } => Some(reason),
            Self::Multithreaded { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebDispatcherSingleThreadedReason {
    BuiltWithoutMultithreadedFeature,
    DisabledByCaller,
    SharedMemoryUnavailable,
    WaitAsyncUnavailable,
    WorkerStartupFailed,
}

#[cfg(feature = "multithreaded")]
fn select_dispatcher_mode(
    allow_threads: bool,
    shared_memory_supported: bool,
    wait_async_supported: bool,
    hardware_concurrency: f64,
) -> WebDispatcherMode {
    if !allow_threads {
        WebDispatcherMode::SingleThreaded {
            reason: WebDispatcherSingleThreadedReason::DisabledByCaller,
        }
    } else if !shared_memory_supported {
        WebDispatcherMode::SingleThreaded {
            reason: WebDispatcherSingleThreadedReason::SharedMemoryUnavailable,
        }
    } else if !wait_async_supported {
        WebDispatcherMode::SingleThreaded {
            reason: WebDispatcherSingleThreadedReason::WaitAsyncUnavailable,
        }
    } else {
        WebDispatcherMode::Multithreaded {
            background_workers: hardware_concurrency.max(MIN_BACKGROUND_THREADS as f64) as usize,
        }
    }
}

#[cfg(not(feature = "multithreaded"))]
fn select_dispatcher_mode(_allow_threads: bool) -> WebDispatcherMode {
    WebDispatcherMode::SingleThreaded {
        reason: WebDispatcherSingleThreadedReason::BuiltWithoutMultithreadedFeature,
    }
}

#[cfg(feature = "multithreaded")]
fn shared_memory_supported() -> bool {
    let global = js_sys::global();
    let has_shared_array_buffer =
        js_sys::Reflect::has(&global, &JsValue::from_str("SharedArrayBuffer")).unwrap_or(false);
    let has_atomics = js_sys::Reflect::has(&global, &JsValue::from_str("Atomics")).unwrap_or(false);
    let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
    let buffer = memory.buffer();
    let is_shared_buffer = buffer.is_instance_of::<js_sys::SharedArrayBuffer>();
    has_shared_array_buffer && has_atomics && is_shared_buffer
}

#[cfg(feature = "multithreaded")]
fn wait_async_supported() -> bool {
    let global = js_sys::global();
    let Ok(atomics) = js_sys::Reflect::get(&global, &JsValue::from_str("Atomics")) else {
        return false;
    };
    let Ok(wait_async) = js_sys::Reflect::get(&atomics, &JsValue::from_str("waitAsync")) else {
        return false;
    };

    wait_async.is_function()
}

#[cfg_attr(not(feature = "multithreaded"), allow(dead_code))]
enum MainThreadItem {
    Runnable {
        runnable: RunnableVariant,
        foreground_queued: bool,
    },
    Delayed {
        runnable: RunnableVariant,
        millis: i32,
    },
    Idle {
        runnable: RunnableVariant,
        timeout: Option<Duration>,
    },
    // Realtime callbacks stay on the main-thread mailbox until a dedicated
    // web audio/worklet execution path exists.
    RealtimeFunction(Box<dyn FnOnce() + Send>),
}

struct MainThreadMailbox {
    sender: PriorityQueueSender<MainThreadItem>,
    #[cfg_attr(not(feature = "multithreaded"), allow(dead_code))]
    receiver: std::sync::Mutex<PriorityQueueReceiver<MainThreadItem>>,
    signal: AtomicI32,
}

impl MainThreadMailbox {
    fn new() -> Self {
        let (sender, receiver) = PriorityQueueReceiver::new();
        Self {
            sender,
            receiver: std::sync::Mutex::new(receiver),
            signal: AtomicI32::new(0),
        }
    }

    fn post(&self, priority: Priority, item: MainThreadItem) -> bool {
        if let Err(item) = self.sender.spin_send(priority, item) {
            std::mem::forget(item);
            log::error!("MainThreadMailbox::send failed: receiver disconnected");
            return false;
        }

        // The queue is the source of truth; this atomic flag is only the
        // multithreaded wake-up signal for the main-thread drain loop.
        let view = self.signal_view();
        js_sys::Atomics::store(&view, 0, 1).ok();
        js_sys::Atomics::notify(&view, 0).ok();
        true
    }

    #[cfg(feature = "multithreaded")]
    fn drain(&self, window: &web_sys::Window) {
        // Only the main thread accesses this receiver; never park if that changes.
        let mut receiver = self
            .receiver
            .try_lock()
            .expect("main-thread mailbox receiver must not be locked");
        loop {
            // We need these `spin` variants because we can't acquire a lock on the main thread.
            match receiver.spin_try_pop() {
                Ok(Some(item)) => execute_on_main_thread(window, item),
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    fn signal_view(&self) -> js_sys::Int32Array {
        let byte_offset = self.signal.as_ptr() as u32;
        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
        js_sys::Int32Array::new_with_byte_offset_and_length(&memory.buffer(), byte_offset, 1)
    }

    #[cfg(feature = "multithreaded")]
    fn run_waker_loop(self: &Arc<Self>, window: web_sys::Window) {
        if !shared_memory_supported() {
            log::warn!("SharedArrayBuffer not available; main thread mailbox waker loop disabled");
            return;
        }

        let mailbox = Arc::clone(self);
        wasm_bindgen_futures::spawn_local(async move {
            let view = mailbox.signal_view();
            loop {
                js_sys::Atomics::store(&view, 0, 0).expect("Atomics.store failed");

                // Items posted between the previous drain and the store above
                // set the signal we just cleared, so their notify is lost.
                // Drain again after re-arming to avoid missing them.
                mailbox.drain(&window);

                let result = match js_sys::Atomics::wait_async(&view, 0, 0) {
                    Ok(result) => result,
                    Err(error) => {
                        log::error!("Atomics.waitAsync failed: {error:?}");
                        break;
                    }
                };

                let is_async = js_sys::Reflect::get(&result, &JsValue::from_str("async"))
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // `async: false` means the signal changed between the store and
                // the wait ("not-equal"): work has already arrived, so skip
                // waiting and drain immediately.
                if is_async {
                    let promise: js_sys::Promise =
                        js_sys::Reflect::get(&result, &JsValue::from_str("value"))
                            .expect("waitAsync result missing 'value'")
                            .unchecked_into();

                    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
                }

                mailbox.drain(&window);
            }
        });
    }
}

#[cfg(feature = "multithreaded")]
fn spawn_background_workers(
    background_workers: usize,
    background_receiver: &PriorityQueueReceiver<RunnableVariant>,
) -> Vec<wasm_thread::JoinHandle<()>> {
    let mut handles = Vec::with_capacity(background_workers);
    for i in 0..background_workers {
        let mut receiver = background_receiver.clone();
        let builder = wasm_thread::Builder::new().name(format!("background-worker-{i}"));
        match builder.spawn(move || {
            loop {
                let runnable: RunnableVariant = match receiver.pop() {
                    Ok(runnable) => runnable,
                    Err(_) => {
                        log::info!("background-worker-{i}: channel disconnected, exiting");
                        break;
                    }
                };

                run_runnable(runnable, false);
            }
        }) {
            Ok(handle) => handles.push(handle),
            Err(error) => log::error!("failed to spawn background-worker-{i}: {error}"),
        }
    }
    handles
}

#[cfg(feature = "multithreaded")]
fn mode_after_worker_startup(started_workers: usize) -> WebDispatcherMode {
    if started_workers == 0 {
        WebDispatcherMode::SingleThreaded {
            reason: WebDispatcherSingleThreadedReason::WorkerStartupFailed,
        }
    } else {
        WebDispatcherMode::Multithreaded {
            background_workers: started_workers,
        }
    }
}

pub struct WebDispatcher {
    main_thread_id: std::thread::ThreadId,
    background_sender: PriorityQueueSender<RunnableVariant>,
    main_thread_mailbox: Arc<MainThreadMailbox>,
    mode: WebDispatcherMode,
    #[cfg(feature = "multithreaded")]
    _background_threads: Vec<wasm_thread::JoinHandle<()>>,
}

impl WebDispatcher {
    pub fn new(browser_window: web_sys::Window, allow_threads: bool) -> Self {
        #[cfg(not(feature = "multithreaded"))]
        let _ = (&browser_window, allow_threads);

        #[cfg(feature = "multithreaded")]
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        #[cfg(not(feature = "multithreaded"))]
        let (background_sender, _) = PriorityQueueReceiver::new();

        let main_thread_mailbox = Arc::new(MainThreadMailbox::new());

        #[cfg(feature = "multithreaded")]
        let mut mode = select_dispatcher_mode(
            allow_threads,
            shared_memory_supported(),
            wait_async_supported(),
            browser_window.navigator().hardware_concurrency(),
        );
        #[cfg(not(feature = "multithreaded"))]
        let mode = select_dispatcher_mode(allow_threads);

        #[cfg(not(feature = "multithreaded"))]
        log::info!("WebDispatcher built without multithreaded support; using single-threaded mode");

        #[cfg(feature = "multithreaded")]
        let background_threads = if let WebDispatcherMode::Multithreaded { background_workers } =
            mode
        {
            // Workers intentionally block on the queue only in explicit
            // multithreaded mode. Stable fallback mode never starts workers.
            let threads = spawn_background_workers(background_workers, &background_receiver);
            mode = mode_after_worker_startup(threads.len());
            match mode {
                WebDispatcherMode::SingleThreaded {
                    reason: WebDispatcherSingleThreadedReason::WorkerStartupFailed,
                } => {
                    log::warn!(
                        "No background workers started; falling back to single-threaded mode"
                    );
                }
                WebDispatcherMode::Multithreaded {
                    background_workers: started_workers,
                } => {
                    if started_workers != background_workers {
                        log::warn!(
                            "Started {started_workers} of {background_workers} requested background workers"
                        );
                    }
                    main_thread_mailbox.run_waker_loop(browser_window.clone());
                }
                WebDispatcherMode::SingleThreaded { reason } => {
                    log::warn!(
                        "Unexpected web worker startup fallback after multithreaded selection: {reason:?}"
                    );
                }
            }
            threads
        } else {
            if allow_threads {
                log::warn!("WebDispatcher using single-threaded mode: {mode:?}");
            }
            Vec::new()
        };

        Self {
            main_thread_id: std::thread::current().id(),
            background_sender,
            main_thread_mailbox,
            mode,
            #[cfg(feature = "multithreaded")]
            _background_threads: background_threads,
        }
    }

    pub fn mode(&self) -> WebDispatcherMode {
        self.mode
    }

    fn on_main_thread(&self) -> bool {
        std::thread::current().id() == self.main_thread_id
    }

    fn dispatch_on_main_thread_inner(
        &self,
        runnable: RunnableVariant,
        priority: Priority,
        foreground_queued: bool,
    ) -> bool {
        if self.on_main_thread() {
            schedule_runnable(&browser_window(), runnable, priority, foreground_queued)
        } else {
            self.main_thread_mailbox.post(
                priority,
                MainThreadItem::Runnable {
                    runnable,
                    foreground_queued,
                },
            )
        }
    }
}

impl PlatformDispatcher for WebDispatcher {
    fn is_main_thread(&self) -> bool {
        self.on_main_thread()
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if !self.mode.supports_background_workers() {
            self.dispatch_on_main_thread_inner(runnable, priority, false);
            return;
        }

        let result = if self.on_main_thread() {
            self.background_sender.spin_send(priority, runnable)
        } else {
            self.background_sender.send(priority, runnable)
        };

        if let Err(error) = result {
            log::error!("dispatch: failed to send to background queue: {error:?}");
        }
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
        self.dispatch_on_main_thread_inner(runnable, priority, true)
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let millis = duration.as_millis().min(i32::MAX as u128) as i32;
        if self.on_main_thread() {
            let callback = Closure::once_into_js(move || {
                run_runnable(runnable, false);
            });
            browser_window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref(),
                    millis,
                )
                .ok();
        } else {
            self.main_thread_mailbox
                .post(Priority::High, MainThreadItem::Delayed { runnable, millis });
        }
    }

    fn spawn_realtime(&self, function: Box<dyn FnOnce() + Send>) {
        if self.on_main_thread() {
            let callback = Closure::once_into_js(move || {
                function();
            });
            browser_window().queue_microtask(callback.unchecked_ref());
        } else {
            self.main_thread_mailbox
                .post(Priority::High, MainThreadItem::RealtimeFunction(function));
        }
    }

    fn dispatch_on_main_thread_when_idle(
        &self,
        runnable: RunnableVariant,
        timeout: Option<Duration>,
    ) -> bool {
        if self.on_main_thread() {
            schedule_idle_runnable(&browser_window(), runnable, timeout)
        } else {
            self.main_thread_mailbox
                .post(Priority::Low, MainThreadItem::Idle { runnable, timeout })
        }
    }

    fn idle_time_remaining(&self) -> Option<Duration> {
        if !self.on_main_thread() {
            return None;
        }
        IDLE_DEADLINE.with(|deadline| {
            deadline
                .borrow()
                .as_ref()
                .map(|deadline| Duration::from_secs_f64(deadline.time_remaining() / 1000.0))
        })
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

fn browser_window() -> web_sys::Window {
    web_sys::window().expect("must be running in a browser window context")
}

#[cfg(feature = "multithreaded")]
fn execute_on_main_thread(window: &web_sys::Window, item: MainThreadItem) {
    match item {
        MainThreadItem::Runnable {
            runnable,
            foreground_queued,
        } => {
            run_runnable(runnable, foreground_queued);
        }
        MainThreadItem::Idle { runnable, timeout } => {
            if !schedule_idle_runnable(window, runnable, timeout) {
                open_gpui::profiler::foreground_task_rejected();
            }
        }
        MainThreadItem::Delayed { runnable, millis } => {
            let callback = Closure::once_into_js(move || {
                run_runnable(runnable, false);
            });
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref(),
                    millis,
                )
                .ok();
        }
        MainThreadItem::RealtimeFunction(function) => {
            function();
        }
    }
}

thread_local! {
    static IDLE_DEADLINE: RefCell<Option<web_sys::IdleDeadline>> = const { RefCell::new(None) };
    static IDLE_CALLBACK_SUPPORTED: Cell<Option<bool>> = const { Cell::new(None) };
}

fn schedule_idle_runnable(
    window: &web_sys::Window,
    runnable: RunnableVariant,
    timeout: Option<Duration>,
) -> bool {
    if !idle_callback_supported(window) {
        return schedule_runnable(window, runnable, Priority::Low, true);
    }
    let callback: Closure<dyn FnMut(web_sys::IdleDeadline)> =
        Closure::once(move |deadline: web_sys::IdleDeadline| {
            let previous = IDLE_DEADLINE.with(|current| current.replace(Some(deadline)));
            let _restore_deadline = defer(move || {
                IDLE_DEADLINE.with(|current| current.replace(previous));
            });
            run_runnable(runnable, true);
        });
    let result = match timeout {
        Some(timeout) => {
            let options = web_sys::IdleRequestOptions::new();
            options.set_timeout(timeout.as_millis().min(u32::MAX as u128) as u32);
            window.request_idle_callback_with_options(callback.as_ref().unchecked_ref(), &options)
        }
        None => window.request_idle_callback(callback.as_ref().unchecked_ref()),
    };
    match result {
        Ok(_) => {
            callback.forget();
            true
        }
        Err(error) => {
            log::error!("requestIdleCallback failed: {error:?}");
            false
        }
    }
}

fn idle_callback_supported(window: &web_sys::Window) -> bool {
    IDLE_CALLBACK_SUPPORTED.with(|supported| {
        if let Some(supported) = supported.get() {
            return supported;
        }
        let probed =
            js_sys::Reflect::has(window.as_ref(), &JsValue::from_str("requestIdleCallback"))
                .unwrap_or(false);
        supported.set(Some(probed));
        probed
    })
}

fn run_runnable(runnable: RunnableVariant, foreground_queued: bool) {
    let metadata = runnable.metadata();
    let _timing = if foreground_queued {
        open_gpui::profiler::task_timing(metadata.spawned, metadata.location)
    } else {
        open_gpui::profiler::task_timing_unqueued(metadata.spawned, metadata.location)
    };
    runnable.run();
}

fn schedule_runnable(
    window: &web_sys::Window,
    runnable: RunnableVariant,
    priority: Priority,
    foreground_queued: bool,
) -> bool {
    let callback: Closure<dyn FnMut()> = Closure::once(move || {
        run_runnable(runnable, foreground_queued);
    });

    let result = match priority {
        Priority::RealtimeAudio => {
            js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("queueMicrotask"))
                .and_then(|function| {
                    function
                        .dyn_into::<js_sys::Function>()?
                        .call1(window.as_ref(), callback.as_ref())
                })
                .map(|_| ())
        }
        _ => {
            // Browser single-threaded scheduling currently preserves only the
            // realtime-vs-deferred distinction. Full priority queue draining is
            // deferred until the web backend has a broader scheduler surface.
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    0,
                )
                .map(|_| ())
        }
    };
    match result {
        Ok(()) => {
            callback.forget();
            true
        }
        Err(error) => {
            log::error!("schedule_runnable failed: {error:?}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    struct MainThreadDropProbe {
        main_thread: std::thread::ThreadId,
        dropped: Rc<Cell<bool>>,
    }

    impl Drop for MainThreadDropProbe {
        fn drop(&mut self) {
            assert_eq!(std::thread::current().id(), self.main_thread);
            self.dropped.set(true);
        }
    }

    fn local_runnable() -> (RunnableVariant, Rc<Cell<bool>>, Rc<Cell<bool>>) {
        let dispatcher = open_gpui::TestDispatcher::new(0);
        let captured = Arc::new(parking_lot::Mutex::new(None));
        let captured_for_dispatch = captured.clone();
        let executor = open_gpui::SchedulerLocalExecutor::new(
            dispatcher.session_id(),
            dispatcher.scheduler().clone(),
            move |runnable| {
                *captured_for_dispatch.lock() = Some(runnable);
            },
        );
        let polled = Rc::new(Cell::new(false));
        let polled_by_future = polled.clone();
        let dropped = Rc::new(Cell::new(false));
        let probe = MainThreadDropProbe {
            main_thread: std::thread::current().id(),
            dropped: dropped.clone(),
        };
        executor
            .spawn(async move {
                let _probe = probe;
                polled_by_future.set(true);
            })
            .detach();
        let runnable = captured.lock().take().expect("runnable should be captured");
        (runnable, polled, dropped)
    }

    fn scheduler_window(method: &str, body: &str) -> web_sys::Window {
        let window = js_sys::Object::new();
        let registration = js_sys::Function::new_with_args("callback, options", body);
        assert!(
            js_sys::Reflect::set(&window, &JsValue::from_str(method), &registration)
                .expect("scheduler method should be installed")
        );
        window.unchecked_into()
    }

    fn registered_callback(window: &web_sys::Window) -> js_sys::Function {
        js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("callback"))
            .expect("registration should capture the callback")
            .unchecked_into()
    }

    #[test]
    fn runnable_registration_retains_ownership_until_execution() {
        for (priority, method) in [
            (Priority::High, "setTimeout"),
            (Priority::RealtimeAudio, "queueMicrotask"),
        ] {
            let window = scheduler_window(method, "this.callback = callback; return 1;");
            let (runnable, polled, dropped) = local_runnable();

            assert!(schedule_runnable(&window, runnable, priority, false));
            assert!(!polled.get());
            assert!(!dropped.get());

            registered_callback(&window)
                .call0(window.as_ref())
                .expect("admitted callback should remain callable");
            assert!(polled.get());
            assert!(dropped.get());
        }
    }

    #[test]
    fn runnable_registration_failure_drops_without_polling() {
        for (priority, method) in [
            (Priority::High, "setTimeout"),
            (Priority::RealtimeAudio, "queueMicrotask"),
        ] {
            let window = scheduler_window(
                method,
                "this.callback = callback; throw new Error('registration rejected');",
            );
            let (runnable, polled, dropped) = local_runnable();

            assert!(!schedule_runnable(&window, runnable, priority, false));
            assert!(!polled.get());
            assert!(dropped.get());
            assert!(registered_callback(&window).call0(window.as_ref()).is_err());
        }
    }

    #[test]
    fn idle_registration_and_fallback_retain_ownership_until_execution() {
        for method in ["requestIdleCallback", "setTimeout"] {
            let previous_support =
                IDLE_CALLBACK_SUPPORTED.with(|supported| supported.replace(None));
            let _restore_support = defer(move || {
                IDLE_CALLBACK_SUPPORTED.with(|supported| supported.set(previous_support));
            });
            let window = scheduler_window(method, "this.callback = callback; return 1;");
            let (runnable, polled, dropped) = local_runnable();

            assert!(schedule_idle_runnable(&window, runnable, None));
            assert!(!polled.get());
            assert!(!dropped.get());

            registered_callback(&window)
                .call1(window.as_ref(), &js_sys::Object::new())
                .expect("admitted callback should remain callable");
            assert!(polled.get());
            assert!(dropped.get());
            IDLE_DEADLINE.with(|deadline| assert!(deadline.borrow().is_none()));
        }
    }

    #[test]
    fn idle_registration_and_fallback_failure_drop_without_polling() {
        for (method, timeout) in [
            ("requestIdleCallback", None),
            ("requestIdleCallback", Some(Duration::from_millis(5))),
            ("setTimeout", None),
        ] {
            let previous_support =
                IDLE_CALLBACK_SUPPORTED.with(|supported| supported.replace(None));
            let _restore_support = defer(move || {
                IDLE_CALLBACK_SUPPORTED.with(|supported| supported.set(previous_support));
            });
            let window = scheduler_window(
                method,
                "this.callback = callback; throw new Error('registration rejected');",
            );
            let (runnable, polled, dropped) = local_runnable();

            assert!(!schedule_idle_runnable(&window, runnable, timeout));
            assert!(!polled.get());
            assert!(dropped.get());
            assert!(
                registered_callback(&window)
                    .call1(window.as_ref(), &js_sys::Object::new())
                    .is_err()
            );
            IDLE_DEADLINE.with(|deadline| assert!(deadline.borrow().is_none()));
        }
    }

    #[cfg(not(feature = "multithreaded"))]
    #[test]
    fn stable_build_reports_missing_multithreaded_feature() {
        assert_eq!(
            select_dispatcher_mode(true),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::BuiltWithoutMultithreadedFeature,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn disabled_threads_report_caller_opt_out() {
        assert_eq!(
            select_dispatcher_mode(false, true, true, 8.0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::DisabledByCaller,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn missing_shared_memory_reports_single_threaded_fallback() {
        assert_eq!(
            select_dispatcher_mode(true, false, true, 8.0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::SharedMemoryUnavailable,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn missing_wait_async_reports_single_threaded_fallback() {
        assert_eq!(
            select_dispatcher_mode(true, true, false, 8.0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::WaitAsyncUnavailable,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn multithreaded_mode_clamps_background_workers() {
        assert_eq!(
            select_dispatcher_mode(true, true, true, 1.0),
            WebDispatcherMode::Multithreaded {
                background_workers: MIN_BACKGROUND_THREADS,
            }
        );

        assert_eq!(
            select_dispatcher_mode(true, true, true, 8.0),
            WebDispatcherMode::Multithreaded {
                background_workers: 8,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn worker_startup_failure_reports_single_threaded_fallback() {
        assert_eq!(
            mode_after_worker_startup(0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::WorkerStartupFailed,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn partial_worker_startup_reports_started_worker_count() {
        assert_eq!(
            mode_after_worker_startup(1),
            WebDispatcherMode::Multithreaded {
                background_workers: 1,
            }
        );
    }
}
