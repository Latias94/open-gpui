#![cfg(not(target_family = "wasm"))]

use open_gpui::TestAppContext;
use open_gpui_tokio::{JoinError, Tokio, init, init_from_handle};
use std::{
    future::pending,
    io,
    rc::Rc,
    sync::mpsc::{self, Sender},
    time::Duration,
};
use tokio::runtime::{Builder, Handle, RuntimeFlavor};

const TIMEOUT: Duration = Duration::from_secs(5);

struct DropNotify(Sender<()>);

impl Drop for DropNotify {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn owned_app() -> TestAppContext {
    let cx = TestAppContext::single();
    cx.executor().allow_parking();
    cx.update(init);
    cx
}

#[test]
fn owned_runtime_exposes_its_handle_and_drives_network_and_time() {
    let cx = owned_app();
    let handle = cx.read(Tokio::handle);
    assert_eq!(handle.runtime_flavor(), RuntimeFlavor::MultiThread);
    assert_eq!(handle.metrics().num_workers(), 2);
    let expected_id = handle.id();
    let task = cx.read(|app| {
        Tokio::spawn(app, async move {
            assert_eq!(Handle::current().id(), expected_id);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let client = tokio::net::TcpStream::connect(address).await?;
            let (server, peer) = listener.accept().await?;
            assert_eq!(client.local_addr()?, peer);
            assert_eq!(server.local_addr()?, address);
            tokio::time::sleep(Duration::from_millis(1)).await;
            Ok::<_, io::Error>(String::from("owned runtime"))
        })
    });
    assert_eq!(
        cx.foreground_executor
            .block_on(task)
            .expect("Tokio join failed")
            .expect("Tokio I/O failed"),
        "owned runtime"
    );
}

#[test]
fn borrowed_handle_keeps_the_external_runtime_owned_by_the_caller() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("external runtime initialization failed");
    let cx = TestAppContext::single();
    cx.executor().allow_parking();
    cx.update(|app| init_from_handle(app, runtime.handle().clone()));
    let handle = cx.read(Tokio::handle);
    let expected_id = runtime.handle().id();
    assert_eq!(handle.id(), expected_id);
    let task = Tokio::spawn(&cx, async move {
        assert_eq!(Handle::current().id(), expected_id);
        String::from("borrowed runtime")
    });
    assert_eq!(
        cx.foreground_executor
            .block_on(task)
            .expect("Tokio join failed"),
        "borrowed runtime"
    );
    let weak_app = Rc::downgrade(&cx.app);
    runtime.block_on(async move { drop(cx) });
    assert!(weak_app.upgrade().is_none());
    assert_eq!(
        runtime
            .block_on(handle.spawn(async { String::from("still running") }))
            .expect("App drop must leave the external runtime running"),
        "still running"
    );
}

#[test]
fn spawn_preserves_application_results_and_reports_panics_as_join_errors() {
    let cx = owned_app();
    let task = Tokio::spawn(&cx, async {
        Err::<(), _>(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "application error",
        ))
    });
    let application_error = cx
        .foreground_executor
        .block_on(task)
        .expect("application errors must not become join errors")
        .expect_err("the task must retain its application error");
    assert_eq!(application_error.kind(), io::ErrorKind::PermissionDenied);
    let task = Tokio::spawn::<_, _, ()>(&cx, async { panic!("bridge panic") });
    let join_error: JoinError = cx
        .foreground_executor
        .block_on(task)
        .expect_err("Tokio must contain the worker panic");
    assert!(join_error.is_panic());
}

#[test]
fn spawn_result_preserves_success_application_errors_and_join_errors() {
    let cx = owned_app();
    let task = Tokio::spawn_result(&cx, async { Ok(String::from("result value")) });
    assert_eq!(
        cx.foreground_executor
            .block_on(task)
            .expect("Tokio result failed"),
        "result value"
    );
    let task = Tokio::spawn_result(&cx, async {
        Err::<(), _>(io::Error::new(io::ErrorKind::PermissionDenied, "application error").into())
    });
    let application_error = cx
        .foreground_executor
        .block_on(task)
        .expect_err("the task must retain its application error");
    assert_eq!(
        application_error
            .downcast_ref::<io::Error>()
            .expect("application error type changed")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    let task = Tokio::spawn_result::<_, _, ()>(&cx, async { panic!("bridge result panic") });
    let join_error = cx
        .foreground_executor
        .block_on(task)
        .expect_err("Tokio must contain the worker panic");
    assert!(
        join_error
            .downcast_ref::<JoinError>()
            .expect("join error type changed")
            .is_panic()
    );
}

#[test]
fn dropping_spawn_task_aborts_tokio_before_and_after_the_bridge_is_polled() {
    let cx = owned_app();
    for poll_bridge in [false, true] {
        let (started, starts) = mpsc::channel();
        let (dropped, drops) = mpsc::channel();
        let task = Tokio::spawn(&cx, async move {
            let _notify = DropNotify(dropped);
            started.send(()).expect("start receiver dropped");
            pending::<()>().await;
        });
        starts
            .recv_timeout(TIMEOUT)
            .expect("Tokio task did not start");
        if poll_bridge {
            cx.run_until_parked();
        }
        drop(task);
        cx.run_until_parked();
        drops
            .recv_timeout(TIMEOUT)
            .expect("dropping the GPUI task must abort Tokio work");
    }
}

#[test]
fn dropping_spawn_result_task_aborts_tokio_before_and_after_the_bridge_is_polled() {
    let cx = owned_app();
    for poll_bridge in [false, true] {
        let (started, starts) = mpsc::channel();
        let (dropped, drops) = mpsc::channel();
        let task = Tokio::spawn_result(&cx, async move {
            let _notify = DropNotify(dropped);
            started.send(()).expect("start receiver dropped");
            pending::<()>().await;
            Ok::<(), anyhow::Error>(())
        });
        starts
            .recv_timeout(TIMEOUT)
            .expect("Tokio task did not start");
        if poll_bridge {
            cx.run_until_parked();
        }
        drop(task);
        cx.run_until_parked();
        drops
            .recv_timeout(TIMEOUT)
            .expect("dropping the GPUI task must abort Tokio work");
    }
}

#[test]
fn app_global_drop_shuts_down_the_owned_runtime_inside_an_async_context() {
    let cx = owned_app();
    let handle = cx.read(Tokio::handle);
    let (started, starts) = mpsc::channel();
    let second_started = started.clone();
    let task = Tokio::spawn(&cx, async move {
        started.send(()).expect("start receiver dropped");
        pending::<()>().await;
    });
    let result_task = Tokio::spawn_result(&cx, async move {
        second_started.send(()).expect("start receiver dropped");
        pending::<()>().await;
        Ok::<(), anyhow::Error>(())
    });
    starts
        .recv_timeout(TIMEOUT)
        .expect("Tokio task did not start");
    starts
        .recv_timeout(TIMEOUT)
        .expect("Tokio result task did not start");
    let foreground = cx.foreground_executor.clone();
    let weak_app = Rc::downgrade(&cx.app);
    handle.block_on(async move { drop(cx) });
    assert!(weak_app.upgrade().is_none());
    assert!(
        foreground
            .block_on(task)
            .expect_err("App drop must stop owned Tokio work")
            .is_cancelled()
    );
    let error = foreground
        .block_on(result_task)
        .expect_err("App drop must stop owned Tokio result work");
    assert!(
        error
            .downcast_ref::<JoinError>()
            .expect("join error type changed")
            .is_cancelled()
    );
}
