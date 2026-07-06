use open_gpui::{
    PlatformDispatcher, Priority, PriorityQueueReceiver, PriorityQueueSender, RunnableVariant,
};
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
    WorkerStartupFailed,
}

#[cfg(feature = "multithreaded")]
fn select_dispatcher_mode(
    allow_threads: bool,
    shared_memory_supported: bool,
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

#[cfg_attr(not(feature = "multithreaded"), allow(dead_code))]
enum MainThreadItem {
    Runnable(RunnableVariant),
    Delayed {
        runnable: RunnableVariant,
        millis: i32,
    },
    // Realtime callbacks stay on the main-thread mailbox until a dedicated
    // web audio/worklet execution path exists.
    RealtimeFunction(Box<dyn FnOnce() + Send>),
}

struct MainThreadMailbox {
    sender: PriorityQueueSender<MainThreadItem>,
    #[cfg_attr(not(feature = "multithreaded"), allow(dead_code))]
    receiver: parking_lot::Mutex<PriorityQueueReceiver<MainThreadItem>>,
    signal: AtomicI32,
}

impl MainThreadMailbox {
    fn new() -> Self {
        let (sender, receiver) = PriorityQueueReceiver::new();
        Self {
            sender,
            receiver: parking_lot::Mutex::new(receiver),
            signal: AtomicI32::new(0),
        }
    }

    fn post(&self, priority: Priority, item: MainThreadItem) {
        if self.sender.spin_send(priority, item).is_err() {
            log::error!("MainThreadMailbox::send failed: receiver disconnected");
        }

        // The queue is the source of truth; this atomic flag is only the
        // multithreaded wake-up signal for the main-thread drain loop.
        let view = self.signal_view();
        js_sys::Atomics::store(&view, 0, 1).ok();
        js_sys::Atomics::notify(&view, 0).ok();
    }

    #[cfg(feature = "multithreaded")]
    fn drain(&self, window: &web_sys::Window) {
        let mut receiver = self.receiver.lock();
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

                if !is_async {
                    log::error!("Atomics.waitAsync returned synchronously; waker loop exiting");
                    break;
                }

                let promise: js_sys::Promise =
                    js_sys::Reflect::get(&result, &JsValue::from_str("value"))
                        .expect("waitAsync result missing 'value'")
                        .unchecked_into();

                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

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

                runnable.run();
            }
        }) {
            Ok(handle) => handles.push(handle),
            Err(error) => log::error!("failed to spawn background-worker-{i}: {error}"),
        }
    }
    handles
}

pub struct WebDispatcher {
    main_thread_id: std::thread::ThreadId,
    browser_window: web_sys::Window,
    background_sender: PriorityQueueSender<RunnableVariant>,
    main_thread_mailbox: Arc<MainThreadMailbox>,
    mode: WebDispatcherMode,
    #[cfg(feature = "multithreaded")]
    _background_threads: Vec<wasm_thread::JoinHandle<()>>,
}

// Safety: `web_sys::Window` is only accessed from the main thread
// All other fields are `Send + Sync` by construction.
unsafe impl Send for WebDispatcher {}
unsafe impl Sync for WebDispatcher {}

impl WebDispatcher {
    pub fn new(browser_window: web_sys::Window, allow_threads: bool) -> Self {
        #[cfg(not(feature = "multithreaded"))]
        let _ = allow_threads;

        #[cfg(feature = "multithreaded")]
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        #[cfg(not(feature = "multithreaded"))]
        let (background_sender, _) = PriorityQueueReceiver::new();

        let main_thread_mailbox = Arc::new(MainThreadMailbox::new());

        #[cfg(feature = "multithreaded")]
        let mut mode = select_dispatcher_mode(
            allow_threads,
            shared_memory_supported(),
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
            if threads.is_empty() {
                mode = WebDispatcherMode::SingleThreaded {
                    reason: WebDispatcherSingleThreadedReason::WorkerStartupFailed,
                };
                log::warn!("No background workers started; falling back to single-threaded mode");
            } else {
                if threads.len() != background_workers {
                    log::warn!(
                        "Started {} of {background_workers} requested background workers",
                        threads.len()
                    );
                    mode = WebDispatcherMode::Multithreaded {
                        background_workers: threads.len(),
                    };
                }
                main_thread_mailbox.run_waker_loop(browser_window.clone());
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
            browser_window,
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
}

impl PlatformDispatcher for WebDispatcher {
    fn is_main_thread(&self) -> bool {
        self.on_main_thread()
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if !self.mode.supports_background_workers() {
            self.dispatch_on_main_thread(runnable, priority);
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

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        if self.on_main_thread() {
            schedule_runnable(&self.browser_window, runnable, priority);
        } else {
            self.main_thread_mailbox
                .post(priority, MainThreadItem::Runnable(runnable));
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let millis = duration.as_millis().min(i32::MAX as u128) as i32;
        if self.on_main_thread() {
            let callback = Closure::once_into_js(move || {
                runnable.run();
            });
            self.browser_window
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
            self.browser_window
                .queue_microtask(callback.unchecked_ref());
        } else {
            self.main_thread_mailbox
                .post(Priority::High, MainThreadItem::RealtimeFunction(function));
        }
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(feature = "multithreaded")]
fn execute_on_main_thread(window: &web_sys::Window, item: MainThreadItem) {
    match item {
        MainThreadItem::Runnable(runnable) => {
            runnable.run();
        }
        MainThreadItem::Delayed { runnable, millis } => {
            let callback = Closure::once_into_js(move || {
                runnable.run();
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

fn schedule_runnable(window: &web_sys::Window, runnable: RunnableVariant, priority: Priority) {
    let callback = Closure::once_into_js(move || {
        runnable.run();
    });
    let callback: &js_sys::Function = callback.unchecked_ref();

    match priority {
        Priority::RealtimeAudio => {
            window.queue_microtask(callback);
        }
        _ => {
            // Browser single-threaded scheduling currently preserves only the
            // realtime-vs-deferred distinction. Full priority queue draining is
            // deferred until the web backend has a broader scheduler surface.
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(callback, 0)
                .ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            select_dispatcher_mode(false, true, 8.0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::DisabledByCaller,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn missing_shared_memory_reports_single_threaded_fallback() {
        assert_eq!(
            select_dispatcher_mode(true, false, 8.0),
            WebDispatcherMode::SingleThreaded {
                reason: WebDispatcherSingleThreadedReason::SharedMemoryUnavailable,
            }
        );
    }

    #[cfg(feature = "multithreaded")]
    #[test]
    fn multithreaded_mode_clamps_background_workers() {
        assert_eq!(
            select_dispatcher_mode(true, true, 1.0),
            WebDispatcherMode::Multithreaded {
                background_workers: MIN_BACKGROUND_THREADS,
            }
        );

        assert_eq!(
            select_dispatcher_mode(true, true, 8.0),
            WebDispatcherMode::Multithreaded {
                background_workers: 8,
            }
        );
    }
}
