#![cfg(feature = "test-support")]

//! Crate-external credential consumers of the fixed Zed target
//! 635026fabc38440fce5e1767d125026a17b12012 and add-only API 79fe6a645f.
//! TestAppContext supplies ready credential stubs, not native secret storage.

use open_gpui::{App, Global, Result, Task, TestAppContext};
use std::{
    cell::Cell,
    future::Future,
    pin::pin,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

fn ready<T: 'static>(task: Task<T>) -> T {
    let mut task = pin!(task);
    match task
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the credential stub must return a ready task"),
    }
}

#[open_gpui::test]
fn public_shared_app_credential_stubs_return_owned_ready_tasks(cx: &mut TestAppContext) {
    let (before, write, after, delete) = cx.read(|app: &App| {
        let url = String::from("open-gpui-test://public-credential-stubs");
        let username = String::from("fixture-account");
        let password = vec![0, 0xff, 0x61, 0];
        let before: Task<Result<Option<(String, Vec<u8>)>>> = app.read_credentials(&url);
        let write: Task<Result<()>> = app.write_credentials(&url, &username, &password);
        let after = app.read_credentials(&url);
        let delete: Task<Result<()>> = app.delete_credentials(&url);
        (before, write, after, delete)
    });

    assert!(before.is_ready(), "the initial stub read must be ready");
    assert!(write.is_ready(), "the stub write must be ready");
    assert!(after.is_ready(), "the later stub read must be ready");
    assert!(delete.is_ready(), "the stub delete must be ready");
    assert!(
        matches!(ready(before), Ok(None)),
        "the stub must report no credentials before a write"
    );
    assert!(ready(write).is_ok(), "the stub write must succeed");
    assert!(
        matches!(ready(after), Ok(None)),
        "the stub must remain empty after a successful stub write"
    );
    assert!(ready(delete).is_ok(), "the stub delete must succeed");
}

struct AppOwner(Rc<Cell<usize>>);

impl Global for AppOwner {}

impl Drop for AppOwner {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn public_credential_tasks_and_async_context_do_not_retain_the_app() {
    let cx = TestAppContext::single();
    let app_drops = Rc::new(Cell::new(0));
    cx.update(|app| app.set_global(AppOwner(Rc::clone(&app_drops))));
    let async_app = cx.to_async();
    let (write, read, delete) = async_app.update(|app| {
        let url = String::from("open-gpui-test://public-credential-lifetime");
        let username = String::from("fixture-account");
        let password = vec![0xff, 0, 0x62];
        (
            app.write_credentials(&url, &username, &password),
            app.read_credentials(&url),
            app.delete_credentials(&url),
        )
    });
    #[cfg(target_os = "macos")]
    let add_only = async_app.update(|app| {
        let url = String::from("open-gpui-test://public-add-only-lifetime");
        let username = String::from("fixture-account");
        let password = vec![0, 0xff, 0x63];
        app.write_credentials_if_not_exists(&url, &username, &password)
    });

    assert!(app_drops.get() == 0, "the live app must own its global");
    cx.quit();
    drop(cx);
    assert!(
        app_drops.get() == 1,
        "retained credential tasks and AsyncApp must not keep the app alive"
    );
    assert!(ready(write).is_ok(), "the owned stub write must complete");
    assert!(
        matches!(ready(read), Ok(None)),
        "the owned stub read must complete without an app"
    );
    assert!(ready(delete).is_ok(), "the owned stub delete must complete");
    #[cfg(target_os = "macos")]
    assert_unsupported(add_only);
    drop(async_app);
    assert!(
        app_drops.get() == 1,
        "the app global must drop exactly once"
    );
}

#[cfg(target_os = "macos")]
fn assert_unsupported(task: Task<Result<bool>>) {
    assert!(task.is_ready(), "the unsupported default must be ready");
    match ready(task) {
        Err(error) => assert!(
            error.to_string() == "this platform does not support atomic credential writes",
            "the default must preserve the official unsupported error"
        ),
        Ok(_) => panic!("the unsupported platform must not report an insertion or duplicate"),
    }
}

#[cfg(target_os = "macos")]
#[open_gpui::test]
fn public_add_only_default_is_unsupported_despite_successful_ordinary_stub_writes(
    cx: &mut TestAppContext,
) {
    let (write, add_only, read) = cx.read(|app: &App| {
        let url = String::from("open-gpui-test://public-add-only-default");
        let username = String::from("fixture-account");
        let password = vec![0, 0xff, 0x64];
        let write = app.write_credentials(&url, &username, &password);
        let add_only: Task<Result<bool>> =
            app.write_credentials_if_not_exists(&url, &username, &password);
        (write, add_only, app.read_credentials(&url))
    });

    assert!(ready(write).is_ok(), "the ordinary stub write must succeed");
    assert_unsupported(add_only);
    assert!(
        matches!(ready(read), Ok(None)),
        "the read stub must remain empty without an atomic fallback"
    );
}
