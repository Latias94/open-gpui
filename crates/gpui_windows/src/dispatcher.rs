use std::{
    ffi::c_void,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
    thread::{ThreadId, current},
    time::Duration,
};

use anyhow::Context;
use open_gpui_util::ResultExt;
use windows::Win32::{
    Foundation::{FILETIME, LPARAM, WPARAM},
    Media::{timeBeginPeriod, timeEndPeriod},
    System::Threading::{
        CloseThreadpoolTimer, CreateThreadpoolTimer, GetCurrentThread, PTP_CALLBACK_INSTANCE,
        PTP_TIMER, SetThreadPriority, SetThreadpoolTimer, THREAD_PRIORITY_TIME_CRITICAL,
        TP_CALLBACK_ENVIRON_V3, TP_CALLBACK_PRIORITY, TP_CALLBACK_PRIORITY_HIGH,
        TP_CALLBACK_PRIORITY_LOW, TP_CALLBACK_PRIORITY_NORMAL, TrySubmitThreadpoolCallback,
    },
    UI::WindowsAndMessaging::PostMessageW,
};

use crate::{HWND, SafeHwnd, WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD};
use open_gpui::{
    PlatformDispatcher, Priority, PriorityQueueSender, RunnableVariant, TimerResolutionGuard,
};

pub(crate) struct WindowsDispatcher {
    pub(crate) wake_posted: AtomicBool,
    main_sender: PriorityQueueSender<RunnableVariant>,
    main_thread_id: ThreadId,
    pub(crate) platform_window_handle: SafeHwnd,
    validation_number: usize,
}

impl WindowsDispatcher {
    pub(crate) fn new(
        main_sender: PriorityQueueSender<RunnableVariant>,
        platform_window_handle: HWND,
        validation_number: usize,
    ) -> Self {
        let main_thread_id = current().id();
        let platform_window_handle = platform_window_handle.into();

        WindowsDispatcher {
            main_sender,
            main_thread_id,
            platform_window_handle,
            validation_number,
            wake_posted: AtomicBool::new(false),
        }
    }

    fn dispatch_on_threadpool(&self, priority: TP_CALLBACK_PRIORITY, runnable: RunnableVariant) {
        let environ = TP_CALLBACK_ENVIRON_V3 {
            Version: 3,
            CallbackPriority: priority,
            Size: size_of::<TP_CALLBACK_ENVIRON_V3>() as u32,
            ..Default::default()
        };
        // SAFETY: The environment is valid for the call. On success the callback takes the
        // runnable's raw reference exactly once. Background dispatch receives Send futures.
        let submission = submit_pool_runnable(runnable, |context| unsafe {
            TrySubmitThreadpoolCallback(Some(run_work_callback), Some(context), Some(&environ))
        });
        if let Err(error) = submission {
            log::error!("Windows thread pool rejected background work; task left pending: {error}");
        }
    }

    fn dispatch_on_threadpool_after(&self, runnable: RunnableVariant, duration: Duration) {
        let due_time = relative_timer_due_time(duration);
        // SAFETY: The context owns a Send timer runnable. On success its one-shot callback
        // consumes it and closes the timer. Win32 copies the due time before returning.
        let submission = submit_pool_runnable(runnable, |context| unsafe {
            let timer = CreateThreadpoolTimer(Some(run_timer_callback), Some(context), None)?;
            SetThreadpoolTimer(timer, Some(&due_time), 0, None);
            Ok(())
        });
        if let Err(error) = submission {
            log::error!("Windows thread pool rejected timer creation; timer left pending: {error}");
        }
    }

    #[inline(always)]
    pub(crate) fn execute_runnable(runnable: RunnableVariant) {
        let location = runnable.metadata().location;
        let spawned = runnable.metadata().spawned;
        let _task_timing = open_gpui::profiler::task_timing(spawned, location);
        runnable.run();
    }
}

impl PlatformDispatcher for WindowsDispatcher {
    fn is_main_thread(&self) -> bool {
        current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        let priority = match priority {
            Priority::RealtimeAudio => {
                panic!("RealtimeAudio priority should use spawn_realtime, not dispatch")
            }
            Priority::High => TP_CALLBACK_PRIORITY_HIGH,
            Priority::Medium => TP_CALLBACK_PRIORITY_NORMAL,
            Priority::Low => TP_CALLBACK_PRIORITY_LOW,
        };
        self.dispatch_on_threadpool(priority, runnable);
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) -> bool {
        match self.main_sender.send(priority, runnable) {
            Ok(_) => {
                if !self.wake_posted.swap(true, Ordering::AcqRel) {
                    unsafe {
                        PostMessageW(
                            Some(self.platform_window_handle.as_raw()),
                            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD,
                            WPARAM(self.validation_number),
                            LPARAM(0),
                        )
                        .log_err();
                    }
                }
                true
            }
            Err(runnable) => {
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
        self.dispatch_on_threadpool_after(runnable, duration);
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            // SAFETY: always safe to call
            let thread_handle = unsafe { GetCurrentThread() };

            // SAFETY: thread_handle is a valid handle to the current thread
            unsafe { SetThreadPriority(thread_handle, THREAD_PRIORITY_TIME_CRITICAL) }
                .context("thread priority")
                .log_err();

            f();
        });
    }

    fn increase_timer_resolution(&self) -> TimerResolutionGuard {
        unsafe {
            timeBeginPeriod(1);
        }
        open_gpui_util::defer(Box::new(|| unsafe {
            timeEndPeriod(1);
        }))
    }
}

/// Transfers the runnable to the native callback. Rejection keeps it pending: closing the
/// infallible scheduler's awaitable could panic an awaiter or falsely fire a timer.
fn submit_pool_runnable(
    runnable: RunnableVariant,
    submit: impl FnOnce(*mut c_void) -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    submit(runnable.into_raw().as_ptr().cast())
}

fn relative_timer_due_time(duration: Duration) -> FILETIME {
    let ticks = duration.as_nanos().div_ceil(100).clamp(1, i64::MAX as u128) as i64;
    let due = (-ticks) as u64;
    FILETIME {
        dwLowDateTime: due as u32,
        dwHighDateTime: (due >> 32) as u32,
    }
}

unsafe extern "system" fn run_work_callback(
    _instance: PTP_CALLBACK_INSTANCE,
    context: *mut c_void,
) {
    // SAFETY: A successful submission invokes this callback once with its owned raw runnable.
    let runnable = unsafe { RunnableVariant::from_raw(NonNull::new_unchecked(context.cast())) };
    WindowsDispatcher::execute_runnable(runnable);
}

unsafe extern "system" fn run_timer_callback(
    _instance: PTP_CALLBACK_INSTANCE,
    context: *mut c_void,
    timer: PTP_TIMER,
) {
    // SAFETY: This is the only callback for the one-shot timer and owns its handle. Closing
    // requests destruction before polling the runnable. Win32 frees it after the callback returns.
    unsafe { CloseThreadpoolTimer(timer) };
    // SAFETY: The timer was armed once with this owned raw runnable as its context.
    let runnable = unsafe { RunnableVariant::from_raw(NonNull::new_unchecked(context.cast())) };
    WindowsDispatcher::execute_runnable(runnable);
}

#[cfg(test)]
mod tests;
