//! Protocol tests require Linux, a windowing feature, and the `dbus-run-session` and
//! `dbus-daemon` executables (provided by the `dbus` package on Debian/Ubuntu).
//! Each test reexecutes itself on a private session bus; no desktop portal is needed.
//! LinuxPlatform supplies the real Linux executor; acquisition uses the portal helpers.

use super::{LinuxPlatform, await_idle_sleep_prevention, inhibit_idle_sleep};
use anyhow::Result;
use futures::{
    channel::oneshot,
    future::{Either, select},
};
use open_gpui::{ActivityGuard, BackgroundExecutor, Platform as _, Task, TestAppContext};
use smol::channel::{Receiver, Sender};
use std::{
    collections::HashMap,
    fs,
    future::Future,
    os::unix::process::CommandExt as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zbus::{
    Connection, ObjectServer,
    message::Header,
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const CHILD_TEST: &str = "OPEN_GPUI_IDLE_SLEEP_PORTAL_CHILD";
const CHILD_REPORT: &str = "OPEN_GPUI_IDLE_SLEEP_PORTAL_REPORT";
const WAIT: Duration = Duration::from_secs(5);
const REASON: &str = "Portal protocol regression";

fn private_session(name: &str, test: impl Future<Output = ()>) {
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::{name}");
    if let Some(child_test) = std::env::var_os(CHILD_TEST) {
        assert_eq!(child_test.as_os_str(), std::ffi::OsStr::new(&name));
        assert!(
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            "the child must use the bus created by dbus-run-session"
        );
        smol::block_on(test);
        fs::write(
            std::env::var_os(CHILD_REPORT).expect("the parent supplies its owned completion path"),
            &name,
        )
        .expect("record the exact executed private test");
        return;
    }

    let report = CompletionReport::new();
    let mut child = Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().expect("locate the current test executable"))
        .args(["--exact", &name, "--nocapture", "--test-threads=1"])
        .env(CHILD_TEST, &name)
        .env(CHILD_REPORT, report.0.join("completed"))
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("DBUS_STARTER_ADDRESS")
        .env_remove("DBUS_STARTER_BUS_TYPE")
        .env("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .expect("portal tests require dbus-run-session and dbus-daemon; install the dbus package");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(
                    status.success(),
                    "private portal test {name} failed: {status}"
                );
                assert_eq!(
                    fs::read_to_string(report.0.join("completed"))
                        .expect("the exact child test must execute and complete"),
                    name,
                );
                return;
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            result => {
                terminate_session(&mut child);
                panic!(
                    "private portal test {name} exceeded its deadline or could not be polled: {result:?}"
                );
            }
        }
    }
}

struct CompletionReport(PathBuf);

impl CompletionReport {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("open-gpui-portal-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("create an exclusively owned private-test directory");
        Self(root)
    }
}

impl Drop for CompletionReport {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("completed"));
        if let Err(error) = fs::remove_dir(&self.0) {
            eprintln!("private portal completion cleanup failed: {error}");
        }
    }
}

fn terminate_session(child: &mut Child) {
    let pid = libc::pid_t::try_from(child.id()).expect("Linux process IDs fit pid_t");
    // SAFETY: process_group(0) placed this runner and its children in their own
    // process group. The negative PID targets only this test's private session.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
    let _ = child.kill();
    let deadline = Instant::now() + Duration::from_secs(2);
    while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn timed_out_private_session_retires_its_runner_and_bus() {
    let mut child = Command::new("dbus-run-session")
        .args(["--", "/bin/sleep", "30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("start the dedicated blocked private session");
    let runner = child.id();
    let cleanup = open_gpui_util::defer(|| {
        terminate_session(&mut child);
    });
    let children_path = format!("/proc/{runner}/task/{runner}/children");
    let started = Instant::now();
    let descendants = loop {
        let children =
            fs::read_to_string(&children_path).expect("the private runner must stay live");
        let children = children
            .split_whitespace()
            .map(|pid| pid.parse::<u32>().unwrap())
            .collect::<Vec<_>>();
        if children.len() >= 2 {
            break children;
        }
        assert!(
            started.elapsed() < WAIT,
            "the private bus and blocked child must start"
        );
        thread::sleep(Duration::from_millis(10));
    };
    drop(cleanup);
    assert!(
        !child
            .try_wait()
            .expect("observe the retired private runner")
            .expect("cleanup must reap its owned runner")
            .success()
    );
    let started = Instant::now();
    loop {
        let live = std::iter::once(runner)
            .chain(descendants.iter().copied())
            .filter(|pid| match fs::metadata(format!("/proc/{pid}")) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => panic!("cannot observe private process {pid}: {error}"),
            })
            .collect::<Vec<_>>();
        if live.is_empty() {
            break;
        }
        assert!(
            started.elapsed() < WAIT,
            "private session processes remain live: {live:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

async fn bounded<T>(description: &str, future: impl Future<Output = T>) -> T {
    let timer = smol::Timer::after(WAIT);
    futures::pin_mut!(future, timer);
    match select(future, timer).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => panic!("timed out waiting for {description}"),
    }
}

struct PendingRequest {
    path: OwnedObjectPath,
    window: String,
    flags: u32,
    reason: String,
}

struct Inhibit {
    requests: Sender<PendingRequest>,
    closes: Sender<()>,
}

#[zbus::interface(name = "org.freedesktop.portal.Inhibit")]
impl Inhibit {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        3
    }

    async fn inhibit(
        &self,
        window: String,
        flags: u32,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let token = options
            .get("handle_token")
            .and_then(|value| <&str>::try_from(value).ok())
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("missing handle_token".into()))?;
        let reason = options
            .get("reason")
            .and_then(|value| <&str>::try_from(value).ok())
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("missing reason".into()))?
            .to_owned();
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".into()))?;
        let sender = sender.as_str().trim_start_matches(':').replace('.', "_");
        let path = OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{token}"
        ))
        .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;
        bounded(
            "Request registration",
            server.at(
                path.clone(),
                PortalRequest {
                    closes: self.closes.clone(),
                },
            ),
        )
        .await?;
        self.requests
            .try_send(PendingRequest {
                path: path.clone(),
                window,
                flags,
                reason,
            })
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        Ok(path)
    }
}

struct PortalRequest {
    closes: Sender<()>,
}

#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl PortalRequest {
    fn close(&self) {
        self.closes.try_send(()).expect("record Request.Close");
    }

    #[zbus(signal)]
    async fn response(
        emitter: &SignalEmitter<'_>,
        response: u32,
        results: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
}

struct Portal {
    connection: Connection,
    requests: Receiver<PendingRequest>,
    closes: Receiver<()>,
}

impl Portal {
    async fn new() -> Self {
        let (requests_tx, requests) = smol::channel::bounded(1);
        let (closes_tx, closes) = smol::channel::unbounded();
        let connection = bounded("private portal startup", async {
            zbus::connection::Builder::session()?
                .name("org.freedesktop.portal.Desktop")?
                .serve_at(
                    "/org/freedesktop/portal/desktop",
                    Inhibit {
                        requests: requests_tx,
                        closes: closes_tx,
                    },
                )?
                .build()
                .await
        })
        .await
        .expect("serve the private portal");
        Self {
            connection,
            requests,
            closes,
        }
    }

    async fn pending(&self) -> PendingRequest {
        let request = bounded("Inhibit", self.requests.recv())
            .await
            .expect("receive Inhibit");
        assert_eq!(request.window, "");
        assert_eq!(request.flags, 4, "only Suspend should be inhibited");
        assert_eq!(request.reason, REASON);
        assert!(
            self.closes.is_empty(),
            "Close must wait for a successful guard"
        );
        request
    }

    async fn succeed(&self, request: &PendingRequest) {
        let emitter = SignalEmitter::new(&self.connection, request.path.clone())
            .expect("create the Request signal emitter");
        bounded(
            "Response",
            PortalRequest::response(&emitter, 0, HashMap::new()),
        )
        .await
        .expect("emit the successful Request.Response (ua{sv})");
    }

    async fn closed_once(&self) {
        bounded("Request.Close", self.closes.recv())
            .await
            .expect("receive Request.Close");
        let duplicate = self.closes.recv();
        let timer = smol::Timer::after(Duration::from_millis(200));
        futures::pin_mut!(duplicate, timer);
        assert!(
            matches!(select(duplicate, timer).await, Either::Right(_)),
            "Request.Close must be called exactly once"
        );
    }
}

fn begin_inhibition(
    executor: &BackgroundExecutor,
) -> (Task<bool>, oneshot::Receiver<Result<ActivityGuard>>) {
    let (guard_tx, guard_rx) = oneshot::channel();
    let request_executor = executor.clone();
    let delivery = executor.spawn(async move {
        guard_tx
            .send(inhibit_idle_sleep(REASON.to_owned(), request_executor).await)
            .is_ok()
    });
    (delivery, guard_rx)
}

#[test]
fn normal_drop_closes_request_once() {
    private_session("normal_drop_closes_request_once", async {
        let portal = Portal::new().await;
        let platform = LinuxPlatform::new(open_gpui::WindowingModes::HEADLESS);
        let executor = platform.background_executor();
        let (delivery, guard_rx) = begin_inhibition(&executor);
        let request = portal.pending().await;
        portal.succeed(&request).await;
        let guard = bounded(
            "idle sleep prevention guard",
            await_idle_sleep_prevention(guard_rx, &executor),
        )
        .await
        .expect("acquire the real portal guard");
        assert!(bounded("guard delivery", delivery).await);
        assert!(
            portal.closes.is_empty(),
            "a live guard must retain inhibition"
        );
        drop(guard);
        portal.closed_once().await;
    });
}

#[test]
fn cancelled_receiver_closes_late_success_once() {
    private_session("cancelled_receiver_closes_late_success_once", async {
        let portal = Portal::new().await;
        let platform = LinuxPlatform::new(open_gpui::WindowingModes::HEADLESS);
        let executor = platform.background_executor();
        let (delivery, guard_rx) = begin_inhibition(&executor);
        let mut waiting = Box::pin(await_idle_sleep_prevention(guard_rx, &executor));
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        let request = portal.pending().await;
        drop(waiting);
        assert!(portal.closes.is_empty());
        portal.succeed(&request).await;
        assert!(!bounded("cancelled guard delivery", delivery).await);
        portal.closed_once().await;
    });
}

#[test]
fn timed_out_receiver_closes_late_success_once() {
    private_session("timed_out_receiver_closes_late_success_once", async {
        let portal = Portal::new().await;
        let platform = LinuxPlatform::new(open_gpui::WindowingModes::HEADLESS);
        let executor = platform.background_executor();
        let cx = TestAppContext::single();
        let (delivery, guard_rx) = begin_inhibition(&executor);
        let mut waiting = Box::pin(await_idle_sleep_prevention(
            guard_rx,
            &cx.background_executor,
        ));
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        let request = portal.pending().await;
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(9));
        cx.run_until_parked();
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        let error = bounded("virtual acquisition timeout", waiting)
            .await
            .err()
            .expect("the receiver should time out before the portal succeeds");
        assert_eq!(
            error.to_string(),
            "Idle sleep prevention acquisition timed out after 10 seconds"
        );
        assert!(portal.closes.is_empty());
        portal.succeed(&request).await;
        assert!(!bounded("timed out guard delivery", delivery).await);
        portal.closed_once().await;
    });
}

#[test]
fn guard_closes_request_after_platform_drop() {
    private_session("guard_closes_request_after_platform_drop", async {
        let portal = Portal::new().await;
        let platform = LinuxPlatform::new(open_gpui::WindowingModes::HEADLESS);
        let executor = platform.background_executor();
        let (delivery, guard_rx) = begin_inhibition(&executor);
        let request = portal.pending().await;
        portal.succeed(&request).await;
        let guard = bounded(
            "idle sleep prevention guard",
            await_idle_sleep_prevention(guard_rx, &executor),
        )
        .await
        .expect("acquire the real portal guard");
        assert!(bounded("guard delivery", delivery).await);
        drop(platform);
        drop(executor);
        assert!(
            portal.closes.is_empty(),
            "the guard should outlive the platform"
        );
        drop(guard);
        portal.closed_once().await;
    });
}
