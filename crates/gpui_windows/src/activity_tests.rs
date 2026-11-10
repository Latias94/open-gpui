use super::{PowerRequest, TEST_ACQUISITION};
use open_gpui::Platform as _;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use windows::Win32::Foundation::{
    CloseHandle, GetHandleInformation, HANDLE, HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS,
    SetHandleInformation,
};

fn release_flag(request: &PowerRequest) -> Arc<AtomicBool> {
    request.reason.released.as_ref().unwrap().clone()
}

fn take_closed() -> Vec<(usize, bool)> {
    TEST_ACQUISITION.with(|probe| std::mem::take(&mut *probe.closed.borrow_mut()))
}

fn live_flags(handle: HANDLE) -> u32 {
    let mut flags = 0;
    // SAFETY: This observes the owned live request handle without adding an object reference.
    unsafe { GetHandleInformation(handle, &mut flags) }.expect("live native power request");
    flags
}

fn close_protected(handle: HANDLE) {
    // SAFETY: Only this test protected the still-live handle whose product close was refused.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0)) }
        .expect("remove the controlled close protection");
    // SAFETY: The original owner could not close this handle; cleanup now completes that close.
    unsafe { CloseHandle(handle) }.expect("close the controlled native request");
}

struct ProtectedRequest {
    request: Option<PowerRequest>,
    handle: HANDLE,
}

impl ProtectedRequest {
    fn new(reason: &str) -> Self {
        let request = PowerRequest::prevent_idle_sleep(reason).expect("native request");
        let handle = request.handle;
        // SAFETY: This test owns the private request handle and changes only its close flag.
        unsafe {
            SetHandleInformation(
                handle,
                HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
            )
        }
        .expect("protect a real owned power request from closing");
        Self {
            request: Some(request),
            handle,
        }
    }
}

impl Drop for ProtectedRequest {
    fn drop(&mut self) {
        drop(self.request.take());
        close_protected(self.handle);
    }
}

#[test]
fn native_power_request_keeps_its_reason_after_the_caller_and_owner_move() {
    let reason = "Scoped 文本 render/export activity 🦀".repeat(32);
    let expected = reason.encode_utf16().chain([0]).collect::<Vec<_>>();
    let request = PowerRequest::prevent_idle_sleep(&reason).expect("native acquisition");
    drop(reason);
    assert_eq!(request.reason.units, expected);
    let buffer = request.reason.units.as_ptr();
    let original = request.handle;
    let released = release_flag(&request);

    let moved = Box::new(request);
    assert_eq!(moved.reason.units.as_ptr(), buffer);
    assert_eq!(moved.reason.units, expected);
    live_flags(original);
    assert!(!released.load(Ordering::SeqCst));
    drop(moved);
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(take_closed(), [(original.0 as usize, true)]);
}

#[test]
fn native_power_requests_have_independent_handle_and_reason_lifetimes() {
    let first = PowerRequest::prevent_idle_sleep("first scope").expect("first native request");
    let second = PowerRequest::prevent_idle_sleep("second scope").expect("second native request");
    let first_handle = first.handle;
    let second_handle = second.handle;
    let first_released = release_flag(&first);
    let second_released = release_flag(&second);
    assert_ne!(first.reason.units.as_ptr(), second.reason.units.as_ptr());
    assert_ne!(first_handle, second_handle);

    drop(first);
    assert!(first_released.load(Ordering::SeqCst));
    assert!(!second_released.load(Ordering::SeqCst));
    assert_eq!(take_closed(), [(first_handle.0 as usize, true)]);
    live_flags(second_handle);
    drop(second);
    assert!(second_released.load(Ordering::SeqCst));
    assert_eq!(take_closed(), [(second_handle.0 as usize, true)]);
}

#[test]
fn native_power_activation_failure_closes_the_real_partial_acquisition() {
    TEST_ACQUISITION.with(|probe| {
        probe.created.set(None);
        probe.fail_set.set(true);
    });
    let error = PowerRequest::prevent_idle_sleep("activation failure")
        .err()
        .expect("the owning activation error must propagate");
    assert_eq!(
        error.to_string(),
        "injected Windows power request activation failure"
    );
    let created = TEST_ACQUISITION.with(|probe| probe.created.take().expect("real created handle"));
    let released =
        TEST_ACQUISITION.with(|probe| probe.reason_released.borrow_mut().take().unwrap());
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(take_closed(), [(created.0 as usize, true)]);
}

#[test]
fn native_public_activity_guard_outlives_its_platform_and_releases_on_a_worker() {
    let platform = crate::WindowsPlatform::new(true).expect("real headless native platform");
    let guard = platform
        .foreground_executor()
        .block_with_timeout(
            Duration::from_secs(2),
            platform.prevent_idle_sleep("worker-owned activity"),
        )
        .unwrap_or_else(|_| panic!("the Windows request must complete synchronously"))
        .expect("public native request");
    let original =
        TEST_ACQUISITION.with(|probe| probe.created.take().expect("real created handle"));
    let released =
        TEST_ACQUISITION.with(|probe| probe.reason_released.borrow_mut().take().unwrap());
    drop(platform);
    live_flags(original);
    assert!(!released.load(Ordering::SeqCst));
    let closed = std::thread::spawn(move || {
        drop(guard);
        take_closed()
    })
    .join()
    .expect("the Send guard must release on its owning worker");
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(closed, [(original.0 as usize, true)]);
}

#[test]
fn native_activity_guard_unwind_retires_the_original_handle() {
    let request = PowerRequest::prevent_idle_sleep("unwind activity").expect("native request");
    let original = request.handle;
    let released = release_flag(&request);
    let guard = open_gpui::ActivityGuard::new(move || drop(request));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("injected scoped activity failure");
    }));
    assert!(panic.is_err());
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(take_closed(), [(original.0 as usize, true)]);
}

#[test]
fn refused_native_close_preserves_the_exact_reason_owner_after_drop() {
    let mut protected = ProtectedRequest::new("retained on native close refusal");
    let released = release_flag(protected.request.as_ref().unwrap());
    drop(protected.request.take());
    assert_eq!(take_closed(), [(protected.handle.0 as usize, false)]);
    assert_ne!(
        live_flags(protected.handle) & HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
        0
    );
    assert!(!released.load(Ordering::SeqCst));
}

#[test]
fn refused_native_close_after_activation_failure_preserves_the_reason() {
    TEST_ACQUISITION.with(|probe| {
        probe.created.set(None);
        probe.closed.borrow_mut().clear();
        probe.fail_set.set(true);
        probe.protect_close.set(true);
    });
    let _cleanup = open_gpui_util::defer(|| {
        TEST_ACQUISITION.with(|probe| {
            if let Some(handle) = probe.created.get()
                && probe.closed.borrow().contains(&(handle.0 as usize, false))
            {
                close_protected(handle);
            }
        });
    });
    let error = PowerRequest::prevent_idle_sleep("failed activation with retained native request")
        .err()
        .expect("controlled activation failure");
    assert_eq!(
        error.to_string(),
        "injected Windows power request activation failure"
    );
    let created =
        TEST_ACQUISITION.with(|probe| probe.created.get().expect("created native handle"));
    let released =
        TEST_ACQUISITION.with(|probe| probe.reason_released.borrow_mut().take().unwrap());
    assert_ne!(live_flags(created) & HANDLE_FLAG_PROTECT_FROM_CLOSE.0, 0);
    assert_eq!(
        TEST_ACQUISITION.with(|probe| probe.closed.borrow().clone()),
        [(created.0 as usize, false)]
    );
    assert!(!released.load(Ordering::SeqCst));
}
