//! Real Credential Manager I/O through App and WindowsPlatform task ownership.
//!
//! Official contracts: zed a0f8fa7bdbbf0269aaf5d5ebfac3ad9adfb576dd and
//! 8720fd1d093963be0aeb36954c15fd1aca1b94d2, crates/gpui_windows/src/platform.rs.
//! Fixed reference: 635026fabc38440fce5e1767d125026a17b12012 at the same path.
//! Storage-only checks are separate from status text rendered by DirectWrite and DirectX.
//! Submitted generations and offscreen GPU readback do not qualify physical input or scanout.

use super::*;
use crate::{
    native_test_observation::NativeWindowTestMessageDisposition,
    util::windows_credentials_target_name,
};
use futures::FutureExt;
use open_gpui::{
    ShapedLine, Task, TextAlign, TextRun, WindowHandle, WindowInitialPresentationStatus, black,
    font,
};
use parking_lot::{Mutex, MutexGuard};
use std::{
    future::Future,
    rc::Weak,
    thread::{self, ThreadId},
};
use windows::Win32::{
    Foundation::ERROR_NOT_FOUND,
    Security::Credentials::{CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_TYPE_GENERIC, CredDeleteW},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) enum CredentialIoOperation {
    Read,
    Write,
    Delete,
}

type CredentialIoEvents = Arc<Mutex<Vec<(CredentialIoOperation, ThreadId)>>>;

static CREDENTIAL_IO_TEST_LOCK: Mutex<()> = Mutex::new(());
static CREDENTIAL_IO_OBSERVER: Mutex<Option<CredentialIoEvents>> = Mutex::new(None);

pub(in crate::platform) fn record_io(operation: CredentialIoOperation) {
    if let Some(events) = CREDENTIAL_IO_OBSERVER.lock().as_ref() {
        events.lock().push((operation, thread::current().id()));
    }
}

struct CredentialIoObservation {
    events: CredentialIoEvents,
    _test_lock: MutexGuard<'static, ()>,
}

impl CredentialIoObservation {
    fn begin() -> Self {
        let test_lock = CREDENTIAL_IO_TEST_LOCK.lock();
        let events = Arc::new(Mutex::new(Vec::new()));
        *CREDENTIAL_IO_OBSERVER.lock() = Some(events.clone());
        Self {
            events,
            _test_lock: test_lock,
        }
    }

    fn assert_operations(&self, expected: &[CredentialIoOperation], foreground: ThreadId) {
        let events = self.events.lock();
        let operations = events
            .iter()
            .map(|(operation, _)| *operation)
            .collect::<Vec<_>>();
        assert_eq!(operations, expected, "native credential operation sequence");
        assert!(
            events.iter().all(|(_, thread)| *thread != foreground),
            "native credential I/O must execute off the foreground thread",
        );
    }
}

impl Drop for CredentialIoObservation {
    fn drop(&mut self) {
        *CREDENTIAL_IO_OBSERVER.lock() = None;
    }
}

struct NativeCredentialItem {
    url: String,
    target: Vec<u16>,
    cleanup_needed: bool,
}

impl NativeCredentialItem {
    fn new() -> Self {
        let url = format!(
            "https://open-gpui-native-credentials.invalid/tests/{}",
            uuid::Uuid::new_v4(),
        );
        let target = windows_credentials_target_name(&url)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        Self {
            url,
            target,
            cleanup_needed: true,
        }
    }

    fn cleanup(&mut self) -> bool {
        if !self.cleanup_needed {
            return true;
        }
        // SAFETY: The null-terminated target belongs exclusively to this UUID-scoped test item.
        let result = unsafe {
            CredDeleteW(
                PCWSTR::from_raw(self.target.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
            )
        };
        let cleaned = match result {
            Ok(()) => true,
            Err(error) => error.code() == ERROR_NOT_FOUND.to_hresult(),
        };
        if cleaned {
            self.cleanup_needed = false;
        }
        cleaned
    }
}

impl Drop for NativeCredentialItem {
    fn drop(&mut self) {
        if !self.cleanup() {
            eprintln!("native credential cleanup failed for the owned test item");
        }
    }
}

fn wait_for_credential_task<T: 'static>(platform: &WindowsPlatform, task: Task<T>) -> T {
    pump_messages_until_with_delay(
        "native credential task completion",
        Duration::from_millis(5),
        || {
            platform.inner.run_foreground_task();
            task.is_ready()
        },
    );
    task.now_or_never()
        .unwrap_or_else(|| panic!("the native credential task must return its completed result"))
}

fn assert_credential_matches(
    actual: Option<(String, Vec<u8>)>,
    expected_username: &str,
    expected_password: &[u8],
) {
    let Some((username, password)) = actual else {
        panic!("the owned credential must be present after a successful write");
    };
    assert!(
        username == expected_username,
        "credential username must round trip"
    );
    assert!(
        password == expected_password,
        "credential bytes must round trip"
    );
}

fn shutdown_credential_app(app: &mut Application) {
    app.update_for_test(|cx| cx.shutdown_for_native_exit_test());
    pump_messages_until_with_delay(
        "native credential App shutdown authority",
        Duration::from_millis(5),
        || app.update_for_test(|cx| cx.native_exit_authority_is_settled_for_test()),
    );
}

#[test]
fn native_credential_io_runs_off_foreground() {
    const USERNAME: &str = "native-credential-thread-test";
    const PASSWORD: &[u8] = b"native-credential-thread-payload";

    discard_stale_quit_messages();
    let observation = CredentialIoObservation::begin();
    let foreground = thread::current().id();
    let mut item = NativeCredentialItem::new();
    let platform = Rc::new(
        WindowsPlatform::new(true).expect("the native credential platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let inner_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);

    let write = app.update_for_test(|cx| cx.write_credentials(&item.url, USERNAME, PASSWORD));
    assert!(
        wait_for_credential_task(&platform, write).is_ok(),
        "credential write must succeed"
    );
    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    let result = wait_for_credential_task(&platform, read)
        .unwrap_or_else(|_| panic!("credential read must succeed"));
    assert_credential_matches(result, USERNAME, PASSWORD);
    let delete = app.update_for_test(|cx| cx.delete_credentials(&item.url));
    assert!(
        wait_for_credential_task(&platform, delete).is_ok(),
        "credential delete must succeed"
    );
    assert!(
        item.cleanup(),
        "the owned native credential must be cleaned up"
    );

    shutdown_credential_app(&mut app);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native credential platform retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && inner_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    observation.assert_operations(
        &[
            CredentialIoOperation::Write,
            CredentialIoOperation::Read,
            CredentialIoOperation::Delete,
        ],
        foreground,
    );
}

#[test]
fn native_credential_app_and_platform_storage_workflow_preserves_task_ownership() {
    const USERNAME: &str = "native-credential-storage-test";
    const PASSWORD: &[u8] = b"native-credential-storage-payload";
    const UPDATED_USERNAME: &str = "native-credential-storage-updated";
    const UPDATED_PASSWORD: &[u8] = b"updated\0native-credential-storage-payload";

    discard_stale_quit_messages();
    let observation = CredentialIoObservation::begin();
    let foreground = thread::current().id();
    let mut item = NativeCredentialItem::new();
    let platform = Rc::new(
        WindowsPlatform::new(true).expect("the native credential storage platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let inner_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);

    let missing = app.update_for_test(|cx| cx.read_credentials(&item.url));
    assert!(
        wait_for_credential_task(&platform, missing)
            .unwrap_or_else(|_| panic!("missing credential read must succeed"))
            .is_none(),
        "an absent owned credential must return None",
    );
    assert!(
        wait_for_credential_task(&platform, platform.delete_credentials(&item.url)).is_err(),
        "deleting a missing owned credential must remain an error",
    );
    let write = app.update_for_test(|cx| cx.write_credentials(&item.url, USERNAME, PASSWORD));
    assert!(
        wait_for_credential_task(&platform, write).is_ok(),
        "credential write must succeed"
    );
    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    assert_credential_matches(
        wait_for_credential_task(&platform, read)
            .unwrap_or_else(|_| panic!("credential read must succeed")),
        USERNAME,
        PASSWORD,
    );
    let update = platform.write_credentials(&item.url, UPDATED_USERNAME, UPDATED_PASSWORD);
    assert!(
        wait_for_credential_task(&platform, update).is_ok(),
        "credential update must succeed"
    );

    let operations_before_preflight = observation.events.lock().len();
    let oversized = vec![0x5a; CRED_MAX_CREDENTIAL_BLOB_SIZE as usize + 1];
    let rejected = app.update_for_test(|cx| cx.write_credentials(&item.url, USERNAME, &oversized));
    assert!(
        rejected
            .now_or_never()
            .unwrap_or_else(|| panic!(
                "oversized credentials must fail before scheduling native I/O"
            ))
            .is_err(),
        "oversized credentials must fail preflight",
    );
    assert_eq!(
        observation.events.lock().len(),
        operations_before_preflight,
        "size preflight must not perform native credential I/O",
    );
    assert_credential_matches(
        wait_for_credential_task(&platform, platform.read_credentials(&item.url))
            .unwrap_or_else(|_| panic!("credential read after rejected update must succeed")),
        UPDATED_USERNAME,
        UPDATED_PASSWORD,
    );
    let delete = app.update_for_test(|cx| cx.delete_credentials(&item.url));
    assert!(
        wait_for_credential_task(&platform, delete).is_ok(),
        "credential delete must succeed"
    );
    let missing = app.update_for_test(|cx| cx.read_credentials(&item.url));
    assert!(
        wait_for_credential_task(&platform, missing)
            .unwrap_or_else(|_| panic!("read after credential deletion must succeed"))
            .is_none(),
        "a deleted owned credential must return None",
    );
    let missing_delete = app.update_for_test(|cx| cx.delete_credentials(&item.url));
    assert!(
        wait_for_credential_task(&platform, missing_delete).is_err(),
        "deleting the owned credential twice must remain an error",
    );
    assert!(
        item.cleanup(),
        "the owned native credential must be cleaned up"
    );

    let survivor = app.update_for_test(|cx| cx.read_credentials(&item.url));
    shutdown_credential_app(&mut app);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "credential Task ownership across native platform shutdown",
        Duration::from_millis(5),
        || {
            survivor.is_ready()
                && !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && inner_lifetime.upgrade().is_none()
        },
    );
    let result = survivor
        .now_or_never()
        .unwrap_or_else(|| panic!("the owned credential Task must complete across shutdown"))
        .unwrap_or_else(|_| panic!("the owned credential read must succeed across shutdown"));
    assert!(
        result.is_none(),
        "the retained Task must preserve missing-item semantics"
    );
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    observation.assert_operations(
        &[
            CredentialIoOperation::Read,
            CredentialIoOperation::Delete,
            CredentialIoOperation::Write,
            CredentialIoOperation::Read,
            CredentialIoOperation::Write,
            CredentialIoOperation::Read,
            CredentialIoOperation::Delete,
            CredentialIoOperation::Read,
            CredentialIoOperation::Delete,
            CredentialIoOperation::Read,
        ],
        foreground,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeCredentialStatus {
    Ready,
    Written,
    Read,
    Updated,
    Verified,
    Deleted,
    Missing,
}

impl NativeCredentialStatus {
    fn text(self) -> &'static str {
        match self {
            Self::Ready => "Credential status: ready",
            Self::Written => "Credential status: written",
            Self::Read => "Credential status: read",
            Self::Updated => "Credential status: updated",
            Self::Verified => "Credential status: verified",
            Self::Deleted => "Credential status: deleted",
            Self::Missing => "Credential status: missing",
        }
    }
}

struct NativeCredentialPaint {
    window_id: WindowId,
    previous_generation: u64,
    status: NativeCredentialStatus,
    shaped: ShapedLine,
}

struct NativeCredentialStatusRoot {
    status: NativeCredentialStatus,
    completed_on: ThreadId,
    completions: Rc<Cell<usize>>,
    paint: Rc<RefCell<Option<NativeCredentialPaint>>>,
    task: Option<Task<()>>,
}

impl Render for NativeCredentialStatusRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let status = self.status;
        let paint = self.paint.clone();
        div().size_full().bg(white()).child(
            canvas(
                move |_, window, _| {
                    let source = status.text();
                    let runs = [TextRun {
                        len: source.len(),
                        font: font("Segoe UI"),
                        color: black(),
                        ..Default::default()
                    }];
                    window
                        .text_system()
                        .shape_line(source.into(), px(16.), &runs, None)
                },
                move |bounds, shaped, window, cx| {
                    let previous_generation = window.rendered_frame_revision();
                    shaped
                        .paint(
                            bounds.origin + point(px(16.), px(16.)),
                            px(24.),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .expect("native credential status text must paint successfully");
                    *paint.borrow_mut() = Some(NativeCredentialPaint {
                        window_id: window.window_handle().window_id(),
                        previous_generation,
                        status,
                        shaped,
                    });
                },
            )
            .size_full(),
        )
    }
}

fn start_native_credential_status_task(
    app: &mut Application,
    handle: WindowHandle<NativeCredentialStatusRoot>,
    status: NativeCredentialStatus,
    operation: impl Future<Output = ()> + 'static,
) -> Weak<()> {
    let foreground = thread::current().id();
    let task_owner = Rc::new(());
    let task_lifetime = Rc::downgrade(&task_owner);
    app.update_for_test(|cx| {
        handle.update(cx, |root, _, cx| {
            assert!(
                root.task.as_ref().is_none_or(|task| task.is_ready()),
                "the preceding credential consumer Task must complete before replacement",
            );
            root.task = Some(cx.spawn(async move |root, cx| {
                operation.await;
                assert!(
                    thread::current().id() == foreground,
                    "credential result consumption must return to the foreground thread",
                );
                root.update(cx, |root, cx| {
                    root.status = status;
                    root.completed_on = thread::current().id();
                    root.completions.set(root.completions.get() + 1);
                    cx.notify();
                })
                .expect("the owned credential consumer must remain live until publication");
                drop(task_owner);
            }));
        })
    })
    .expect("the native credential status root must own its consumer Task");
    task_lifetime
}

fn wait_for_native_credential_submission(
    app: &mut Application,
    platform: &WindowsPlatform,
    handle: WindowHandle<NativeCredentialStatusRoot>,
    status: NativeCredentialStatus,
    previous_generation: u64,
) -> u64 {
    let foreground = thread::current().id();
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native credential status accepted nonempty Submitted frame",
        Duration::from_millis(5),
        || {
            platform.inner.run_foreground_task();
            submitted = app
                .update_for_test(|cx| {
                    handle.update(cx, |root, window, _| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let diagnostics = window.submitted_frame_diagnostics_for_test()?;
                        let paint = root.paint.borrow();
                        let paint = paint.as_ref()?;
                        if !(root.status == status
                            && root.completed_on == foreground
                            && root.task.as_ref().is_none_or(|task| task.is_ready())
                            && paint.window_id == handle.window_id()
                            && paint.status == status
                            && paint.shaped.text.as_ref() == status.text()
                            && paint.shaped.width > px(0.)
                            && paint.shaped.runs.iter().any(|run| !run.glyphs.is_empty())
                            && paint.previous_generation.checked_add(1) == Some(attempt.generation)
                            && attempt.generation > previous_generation
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && diagnostics.generation() == attempt.generation)
                        {
                            return None;
                        }
                        let capture = window.capture_frame();
                        assert!(
                            capture.unsupported_reason.is_none(),
                            "native credential status GPU readback must be supported",
                        );
                        assert!(
                            capture.metadata.window_id == handle.window_id()
                                && capture.metadata.framework_frame_generation
                                    == attempt.generation,
                            "status pixels must belong to the exact submitted window generation",
                        );
                        let image = capture
                            .image
                            .as_ref()
                            .expect("native credential status pixels must be available");
                        assert!(
                            image
                                .pixels()
                                .any(|pixel| pixel.0[..3].iter().all(|channel| *channel < 192)),
                            "the accepted status frame must contain visible text pixels",
                        );
                        Some(attempt.generation)
                    })
                })
                .expect("the native credential status window must remain live until submission");
            submitted.is_some()
        },
    );
    submitted.expect("credential status must reach an accepted nonempty Submitted generation")
}

#[test]
fn native_credential_background_io_submits_status_and_retires_owned_task_and_window() {
    const USERNAME: &str = "native-credential-visible-test";
    const PASSWORD: &[u8] = b"native-credential-visible-payload";
    const UPDATED_USERNAME: &str = "native-credential-visible-updated";
    const UPDATED_PASSWORD: &[u8] = b"updated\0native-credential-visible-payload";

    discard_stale_quit_messages();
    let io_observation = CredentialIoObservation::begin();
    let foreground = thread::current().id();
    let mut item = NativeCredentialItem::new();
    let (_window_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new(false)
            .expect("the native credential DirectWrite and DirectX platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let inner_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let platform_text_lifetime = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let app_text_lifetime = Arc::downgrade(&app.text_system());
    platform.begin_vsync_thread();

    let paint = Rc::new(RefCell::new(None));
    let completions = Rc::new(Cell::new(0));
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(640.), px(160.)), cx)),
                    focus_on_appearing: false,
                    activation_policy: WindowActivationPolicy {
                        accepts_activation: false,
                        focus_on_click: false,
                    },
                    accepts_pointer_input: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|_| NativeCredentialStatusRoot {
                        status: NativeCredentialStatus::Ready,
                        completed_on: foreground,
                        completions: completions.clone(),
                        paint: paint.clone(),
                        task: None,
                    })
                },
            )
        })
        .expect("the native credential status window must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("the native credential status window must own its exact registration");
    let hwnd = registration.as_raw();
    let native = platform
        .window_from_hwnd(hwnd)
        .expect("the native credential status HWND must have its native owner");
    assert!(
        native.registration == registration,
        "the native credential status owner must match its exact registration",
    );
    let native_lifetime = Rc::downgrade(&native);
    let native_facts = native
        .observed_platform_facts_for_test()
        .expect("native credential status window facts must be observable");
    assert!(
        !native_facts.accepts_pointer_input
            && !native_facts.accepts_activation
            && !native_facts.focus_on_click,
        "the credential status window must preserve its input policy",
    );
    let frame_requests = observe_frame_requests(&native);
    let (root_lifetime, window_text_lifetime) = app.update_for_test(|cx| {
        let root = handle
            .root(cx)
            .expect("the native credential status root must exist")
            .downgrade();
        let text = handle
            .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
            .expect("the native credential window must expose its own TextSystem");
        (root, text)
    });
    let initial_paint_cursor = observation.cursor();
    assert!(
        unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() },
        "the native credential status window must accept an initial paint request",
    );
    pump_messages_until_with_delay(
        "native credential status initialization WM_PAINT",
        Duration::from_millis(5),
        || {
            observation.any_after(initial_paint_cursor, |event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::Other(WM_PAINT),
                            disposition: NativeWindowTestMessageDisposition::Returned(0),
                            ..
                        }
                    )
            })
        },
    );
    let mut generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Ready,
        0,
    );
    let requests_before_io = frame_requests.get();

    let write = app.update_for_test(|cx| cx.write_credentials(&item.url, USERNAME, PASSWORD));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Written,
        async move {
            assert!(write.await.is_ok(), "credential write must succeed");
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Written,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );

    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Read,
        async move {
            assert_credential_matches(
                read.await
                    .unwrap_or_else(|_| panic!("credential read must succeed")),
                USERNAME,
                PASSWORD,
            );
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Read,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );

    let update = app
        .update_for_test(|cx| cx.write_credentials(&item.url, UPDATED_USERNAME, UPDATED_PASSWORD));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Updated,
        async move {
            assert!(update.await.is_ok(), "credential update must succeed");
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Updated,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );

    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Verified,
        async move {
            assert_credential_matches(
                read.await
                    .unwrap_or_else(|_| panic!("updated credential read must succeed")),
                UPDATED_USERNAME,
                UPDATED_PASSWORD,
            );
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Verified,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );

    let delete = app.update_for_test(|cx| cx.delete_credentials(&item.url));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Deleted,
        async move {
            assert!(delete.await.is_ok(), "credential delete must succeed");
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Deleted,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );

    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    let task_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Missing,
        async move {
            assert!(
                read.await
                    .unwrap_or_else(|_| panic!("missing credential read must succeed"))
                    .is_none(),
                "the deleted owned credential must return None",
            );
        },
    );
    generation = wait_for_native_credential_submission(
        &mut app,
        &platform,
        handle,
        NativeCredentialStatus::Missing,
        generation,
    );
    assert!(
        task_lifetime.upgrade().is_none(),
        "the completed credential consumer must release its future"
    );
    assert!(
        generation > 0,
        "the credential workflow must reach an accepted generation"
    );
    assert_eq!(
        completions.get(),
        6,
        "each credential result must publish exactly once"
    );
    assert!(
        frame_requests.get() > requests_before_io,
        "credential completion must request status frames"
    );
    assert!(
        item.cleanup(),
        "the owned native credential must be cleaned up"
    );

    let read = app.update_for_test(|cx| cx.read_credentials(&item.url));
    let read_consumed = Rc::new(Cell::new(false));
    let consumed = read_consumed.clone();
    let pending_lifetime = start_native_credential_status_task(
        &mut app,
        handle,
        NativeCredentialStatus::Read,
        async move {
            assert!(
                read.await
                    .unwrap_or_else(|_| panic!("retirement credential read must succeed"))
                    .is_none(),
                "the retirement read must preserve missing-item semantics",
            );
            consumed.set(true);
            futures::future::pending::<()>().await;
        },
    );
    pump_messages_until_with_delay(
        "owned credential consumer waits after actual native I/O",
        Duration::from_millis(5),
        || {
            platform.inner.run_foreground_task();
            read_consumed.get()
        },
    );
    assert!(
        pending_lifetime.upgrade().is_some(),
        "the pending consumer Task must own its future"
    );
    app.update_for_test(|cx| {
        handle.update(cx, |root, window, _| {
            assert!(
                root.status == NativeCredentialStatus::Missing
                    && root.task.as_ref().is_some_and(|task| !task.is_ready())
                    && window.rendered_frame_revision() >= generation,
                "pending credential consumption must preserve the submitted status",
            );
        })
    })
    .expect("the pending credential consumer root must remain live until retirement");

    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native credential status window must close through its public owner");
    pump_messages_until_with_delay(
        "exact credential HWND registration root Task and TextSystem retirement",
        Duration::from_millis(5),
        || {
            platform.inner.run_foreground_task();
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
                && pending_lifetime.upgrade().is_none()
                && window_text_lifetime.upgrade().is_none()
        },
    );
    let lifecycle = observation
        .events_after(observation_cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert!(
                    disposition != NativeWindowTestMessageDisposition::Panicked,
                    "the credential native boundary must not swallow a panic"
                );
                None
            }
            lifecycle => Some(lifecycle),
        })
        .collect::<Vec<_>>();
    let [
        NativeWindowTestEventKind::PresentationQuiesced {
            ticket_generation: quiesced,
        },
        NativeWindowTestEventKind::DestroyEntered {
            ticket_generation: destroyed,
        },
        NativeWindowTestEventKind::NativeTerminal {
            ticket_generation: terminal,
        },
    ] = lifecycle.as_slice()
    else {
        panic!(
            "credential retirement must quiesce destroy and terminal its exact native generation"
        );
    };
    assert!(
        quiesced == destroyed && destroyed == terminal,
        "credential retirement must use one ticket generation"
    );
    app.update_for_test(|cx| {
        assert!(
            handle.update(cx, |_, _, _| ()).is_err(),
            "the retired credential window must reject updates"
        );
        assert!(
            cx.windows().is_empty(),
            "the credential App must own no retired window"
        );
    });
    assert!(
        platform.raw_window_handles.read().is_empty(),
        "credential native registrations must retire"
    );
    assert_eq!(
        completions.get(),
        6,
        "retirement must cancel pending status publication"
    );
    drop(native);
    assert!(
        native_lifetime.upgrade().is_none(),
        "the retired credential HWND owner must release"
    );
    let paint_lifetime = Rc::downgrade(&paint);
    drop(paint);
    assert!(
        paint_lifetime.upgrade().is_none(),
        "retired credential render closures must release"
    );

    shutdown_credential_app(&mut app);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native credential App platform and text resource retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && inner_lifetime.upgrade().is_none()
                && app_text_lifetime.upgrade().is_none()
                && platform_text_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert_eq!(
        completions.get(),
        6,
        "shutdown must not revive a retired credential consumer"
    );
    io_observation.assert_operations(
        &[
            CredentialIoOperation::Write,
            CredentialIoOperation::Read,
            CredentialIoOperation::Write,
            CredentialIoOperation::Read,
            CredentialIoOperation::Delete,
            CredentialIoOperation::Read,
            CredentialIoOperation::Read,
        ],
        foreground,
    );
}
