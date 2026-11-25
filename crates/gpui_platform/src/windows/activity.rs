use anyhow::{Context as _, Result};
use open_gpui::ActivityGuard;
use open_gpui_util::ResultExt as _;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Power::{
                PowerClearRequest, PowerCreateRequest, PowerRequestSystemRequired, PowerSetRequest,
            },
            SystemServices::POWER_REQUEST_CONTEXT_VERSION,
            Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
        },
    },
    core::PWSTR,
};

pub(super) fn prevent_idle_sleep(reason: &str) -> Result<ActivityGuard> {
    PowerRequest::prevent_idle_sleep(reason)
        .map(|request| ActivityGuard::new(move || drop(request)))
}

struct PowerRequest {
    handle: HANDLE,
    reason: PowerReason,
}

// SAFETY: The request is a thread-safe kernel handle and owns its retained UTF-16 reason buffer.
unsafe impl Send for PowerRequest {}

/// Owns the exact allocation borrowed by Windows until its request is closed.
#[derive(Default)]
struct PowerReason {
    units: Vec<u16>,
    #[cfg(test)]
    released: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl PowerReason {
    fn new(reason: &str) -> Self {
        Self {
            units: reason.encode_utf16().chain([0]).collect(),
            #[cfg(test)]
            released: Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            ))),
        }
    }
}

#[cfg(test)]
impl Drop for PowerReason {
    fn drop(&mut self) {
        if let Some(released) = &self.released {
            released.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl PowerRequest {
    fn prevent_idle_sleep(reason: &str) -> Result<Self> {
        let mut reason = PowerReason::new(reason);
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.units.as_mut_ptr()),
            },
        };
        let handle = unsafe { PowerCreateRequest(&context) }
            .context("Failed to create a Windows power request")?;
        #[cfg(test)]
        TEST_ACQUISITION.with(|probe| {
            probe.created.set(Some(handle));
            *probe.reason_released.borrow_mut() = reason.released.clone();
        });
        if let Err(error) = set_system_required(handle) {
            close_request(handle, reason)
                .context("Failed to close the Windows power request")
                .log_err();
            return Err(error);
        }
        Ok(Self { handle, reason })
    }
}

fn set_system_required(handle: HANDLE) -> Result<()> {
    #[cfg(test)]
    {
        let (protect_close, fail_set) = TEST_ACQUISITION.with(|probe| {
            (
                probe.protect_close.replace(false),
                probe.fail_set.replace(false),
            )
        });
        if protect_close {
            // SAFETY: This is the real handle created for the current controlled acquisition.
            unsafe {
                windows::Win32::Foundation::SetHandleInformation(
                    handle,
                    windows::Win32::Foundation::HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                    windows::Win32::Foundation::HANDLE_FLAG_PROTECT_FROM_CLOSE,
                )
            }
            .context("protecting the controlled power-request handle")?;
        }
        if fail_set {
            anyhow::bail!("injected Windows power request activation failure");
        }
    }
    unsafe { PowerSetRequest(handle, PowerRequestSystemRequired) }
        .context("Failed to set the Windows power request")
}

impl Drop for PowerRequest {
    fn drop(&mut self) {
        let cleared = unsafe { PowerClearRequest(self.handle, PowerRequestSystemRequired) };
        let closed = close_request(self.handle, std::mem::take(&mut self.reason));
        cleared
            .context("Failed to clear the Windows power request")
            .log_err();
        closed
            .context("Failed to close the Windows power request")
            .log_err();
    }
}

fn close_request(handle: HANDLE, reason: PowerReason) -> windows::core::Result<()> {
    let result = unsafe { CloseHandle(handle) };
    if result.is_err() {
        std::mem::forget(reason);
    }
    #[cfg(test)]
    TEST_ACQUISITION.with(|probe| {
        probe
            .closed
            .borrow_mut()
            .push((handle.0 as usize, result.is_ok()));
    });
    result
}

#[cfg(test)]
#[derive(Default)]
struct AcquisitionProbe {
    created: std::cell::Cell<Option<HANDLE>>,
    fail_set: std::cell::Cell<bool>,
    protect_close: std::cell::Cell<bool>,
    reason_released: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    closed: std::cell::RefCell<Vec<(usize, bool)>>,
}

#[cfg(test)]
thread_local! {
    static TEST_ACQUISITION: AcquisitionProbe = AcquisitionProbe::default();
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
