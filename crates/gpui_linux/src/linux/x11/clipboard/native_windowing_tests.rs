use super::{
    CLIPBOARDS, Clipboard, ClipboardKind, HANDOVERS, Inner, WaitConfig,
    wait_for_clipboard_handovers,
};
use anyhow::{Context as _, Result, ensure};
use parking_lot::Mutex;
use std::{
    borrow::Cow,
    env,
    io::{BufRead as _, BufReader},
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection as _,
    protocol::{
        Event,
        xproto::{
            ConnectionExt as _, EventMask, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, Time,
        },
    },
};

const CHILD_TEST: &str = "OPEN_GPUI_X11_CLIPBOARD_NATIVE_CHILD";
const DISPLAY_A: &str = "OPEN_GPUI_X11_CLIPBOARD_DISPLAY_A";
const DISPLAY_B: &str = "OPEN_GPUI_X11_CLIPBOARD_DISPLAY_B";
const COMPLETE: &str = "PRIVATE_X11_CLIPBOARD_PARTITION_AND_HANDOVER_COMPLETE";

/// Requires /usr/bin/Xvfb and the X11 feature, without a desktop or clipboard manager.
/// Both servers use random nonzero display numbers confirmed by -displayfd. No live process
/// display environment is modified. The parent owns and reaps both servers if its child fails.
#[test]
#[ignore = "requires /usr/bin/Xvfb; creates two private X11 servers and selection managers"]
fn native_clipboard_display_partition_and_handover() {
    let name = module_path!().split_once("::").unwrap().1;
    let name = format!("{name}::native_clipboard_display_partition_and_handover");
    if let Some(child_test) = env::var_os(CHILD_TEST) {
        assert_eq!(child_test.as_os_str(), std::ffi::OsStr::new(&name));
        let a = env::var(DISPLAY_A).expect("the parent supplies private display A");
        let b = env::var(DISPLAY_B).expect("the parent supplies private display B");
        assert_ne!(a, b);
        for display in [&a, &b] {
            let number = display.strip_prefix(':').unwrap().parse::<u32>().unwrap();
            assert_ne!(number, 0, "the probe must never use display :0");
        }
        assert!(env::var_os("DISPLAY").is_none());
        qualify_partition_and_handover(&a, &b);
        println!("{COMPLETE}");
        return;
    }

    let a = PrivateXvfb::start().expect("start private Xvfb A with -displayfd");
    let b = PrivateXvfb::start().expect("start private Xvfb B with -displayfd");
    assert_ne!(a.display, b.display);
    let mut child = OwnedProcess::new(
        Command::new(env::current_exe().expect("locate the current test executable"))
            .args([
                "--exact",
                &name,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_TEST, &name)
            .env(DISPLAY_A, &a.display)
            .env(DISPLAY_B, &b.display)
            .env_remove("DISPLAY")
            .env_remove("XAUTHORITY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start the isolated clipboard protocol probe"),
    );
    let deadline = Instant::now() + Duration::from_secs(45);
    while child
        .child()
        .try_wait()
        .expect("poll the protocol probe")
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "private clipboard probe exceeded 45 seconds"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.output();
    assert!(
        output.status.success(),
        "private clipboard probe failed: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(COMPLETE));
}

struct OwnedProcess(Option<Child>);

impl OwnedProcess {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn child(&mut self) -> &mut Child {
        self.0.as_mut().expect("the process has not been consumed")
    }

    fn output(mut self) -> Output {
        self.0
            .take()
            .unwrap()
            .wait_with_output()
            .expect("reap the protocol probe")
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            if let Ok(pid) = libc::pid_t::try_from(child.id()) {
                // SAFETY: this guard owns the unreaped child; its PID cannot have been reused.
                unsafe { libc::kill(pid, libc::SIGTERM) };
                let deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < deadline {
                    match child.try_wait() {
                        Ok(Some(_)) => return,
                        Ok(None) => thread::sleep(Duration::from_millis(5)),
                        Err(_) => break,
                    }
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct PrivateXvfb {
    _process: OwnedProcess,
    display: String,
}

impl PrivateXvfb {
    fn start() -> Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_read_timeout(Some(Duration::from_secs(5)))?;
        let stdout: OwnedFd = writer.into();
        let random = uuid::Uuid::new_v4();
        let random = u16::from_le_bytes([random.as_bytes()[0], random.as_bytes()[1]]);
        let candidate = 1024 + u32::from(random) % 50000;
        let process = OwnedProcess::new(
            Command::new("/usr/bin/Xvfb")
                .arg(format!(":{candidate}"))
                .args([
                    "-displayfd",
                    "1",
                    "-screen",
                    "0",
                    "320x240x24",
                    "-nolisten",
                    "tcp",
                    "-noreset",
                    "-ac",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::inherit())
                .spawn()
                .context("/usr/bin/Xvfb must be installed")?,
        );
        let mut display_number = String::new();
        BufReader::new(reader)
            .read_line(&mut display_number)
            .context("Xvfb must publish its allocated display within five seconds")?;
        let number = display_number.trim().parse::<u32>()?;
        ensure!(
            number == candidate,
            "Xvfb must confirm the exact privately requested nonzero display"
        );
        let display = format!(":{number}");
        Ok(Self {
            _process: process,
            display,
        })
    }
}

struct PrivateClipboardManager {
    stop: Arc<AtomicBool>,
    saved: Arc<Mutex<Vec<Vec<u8>>>>,
    inner: Weak<Inner>,
    window: u32,
    worker: Option<JoinHandle<Result<()>>>,
}

impl PrivateClipboardManager {
    fn start(display: &str) -> Result<Self> {
        let inner = Arc::new(Inner::new(display)?);
        let window = inner.server.win_id;
        inner
            .server
            .conn
            .set_selection_owner(window, inner.atoms.CLIPBOARD_MANAGER, Time::CURRENT_TIME)?
            .check()?;
        ensure!(
            inner
                .server
                .conn
                .get_selection_owner(inner.atoms.CLIPBOARD_MANAGER)?
                .reply()?
                .owner
                == window,
            "the private manager must own CLIPBOARD_MANAGER"
        );
        let stop = Arc::new(AtomicBool::new(false));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let weak = Arc::downgrade(&inner);
        let worker = thread::Builder::new()
            .name("Private clipboard manager".into())
            .spawn({
                let stop = stop.clone();
                let saved = saved.clone();
                move || {
                    let deadline = Instant::now() + Duration::from_secs(40);
                    while !stop.load(Ordering::Acquire) {
                        ensure!(
                            Instant::now() < deadline,
                            "private clipboard manager timed out"
                        );
                        match inner.server.conn.poll_for_event()? {
                            Some(Event::SelectionRequest(event))
                                if event.selection == inner.atoms.CLIPBOARD_MANAGER =>
                            {
                                ensure!(
                                    event.target == inner.atoms.SAVE_TARGETS,
                                    "unexpected manager request"
                                );
                                ensure!(
                                    !inner.is_owner(ClipboardKind::Clipboard)?,
                                    "the manager must read the retiring owner through X11"
                                );
                                let data = inner
                                    .read(&[inner.atoms.UTF8_STRING], ClipboardKind::Clipboard)?;
                                saved.lock().push(data.bytes.clone());
                                inner.write(
                                    vec![data],
                                    ClipboardKind::Clipboard,
                                    WaitConfig::None,
                                )?;
                                inner
                                    .server
                                    .conn
                                    .send_event(
                                        false,
                                        event.requestor,
                                        EventMask::NO_EVENT,
                                        SelectionNotifyEvent {
                                            response_type: SELECTION_NOTIFY_EVENT,
                                            sequence: event.sequence,
                                            time: event.time,
                                            requestor: event.requestor,
                                            selection: event.selection,
                                            target: event.target,
                                            property: event.property,
                                        },
                                    )?
                                    .check()?;
                            }
                            Some(Event::SelectionRequest(event)) => {
                                inner.handle_selection_request(event)?;
                            }
                            Some(_) => {}
                            None => thread::sleep(Duration::from_millis(1)),
                        }
                    }
                    Ok(())
                }
            })?;
        Ok(Self {
            stop,
            saved,
            inner: weak,
            window,
            worker: Some(worker),
        })
    }

    fn finish(mut self) -> Vec<Vec<u8>> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .unwrap()
            .join()
            .expect("join the private manager")
            .expect("private manager protocol");
        assert!(
            self.inner.upgrade().is_none(),
            "the manager thread must release its native context"
        );
        let saved = self.saved.lock().clone();
        saved
    }
}

impl Drop for PrivateClipboardManager {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn protocol_text(reader: &Inner, selection: ClipboardKind) -> String {
    assert!(
        !reader.is_owner(selection).unwrap(),
        "the read must cross a native selection connection"
    );
    let data = reader
        .read(&[reader.atoms.UTF8_STRING], selection)
        .expect("read the native selection");
    assert_eq!(data.format, reader.atoms.UTF8_STRING);
    String::from_utf8(data.bytes).expect("the selection contains the exact UTF-8 payload")
}

fn assert_released(display: &str, owner: &Weak<Inner>, reader: &Inner, window: u32) {
    assert!(!CLIPBOARDS.lock().contains_key(display));
    assert!(
        owner.upgrade().is_none(),
        "joined clipboard threads must release their Inner"
    );
    assert!(
        reader
            .server
            .conn
            .get_window_attributes(window)
            .unwrap()
            .reply()
            .is_err(),
        "the retired clipboard window must be destroyed"
    );
}

fn qualify_partition_and_handover(a: &str, b: &str) {
    assert!(CLIPBOARDS.lock().is_empty());
    assert!(HANDOVERS.lock().is_empty());
    let manager_a = PrivateClipboardManager::start(a).expect("own private manager A");
    let manager_b = PrivateClipboardManager::start(b).expect("own private manager B");
    let reader_a = Inner::new(a).expect("independent native reader A");
    let reader_b = Inner::new(b).expect("independent native reader B");
    let clipboard_a = Clipboard::new(a).expect("clipboard A");
    let clipboard_b = Clipboard::new(b).expect("clipboard B");
    let shared_a = Clipboard::new(a).expect("second handle for A");
    let shared_b = Clipboard::new(b).expect("second handle for B");
    assert!(Arc::ptr_eq(&clipboard_a.inner, &shared_a.inner));
    assert!(Arc::ptr_eq(&clipboard_b.inner, &shared_b.inner));
    assert!(!Arc::ptr_eq(&clipboard_a.inner, &clipboard_b.inner));
    assert_eq!(CLIPBOARDS.lock().len(), 2);

    for (selection, label) in [
        (ClipboardKind::Clipboard, "clipboard"),
        (ClipboardKind::Primary, "primary"),
        (ClipboardKind::Secondary, "secondary"),
    ] {
        let text_a = format!("display A {label} 文本 🦀");
        let text_b = format!("display B {label} différent 🦀");
        clipboard_a
            .set_text(Cow::Borrowed(&text_a), selection, WaitConfig::None)
            .unwrap();
        clipboard_b
            .set_text(Cow::Borrowed(&text_b), selection, WaitConfig::None)
            .unwrap();
        assert_eq!(protocol_text(&reader_a, selection), text_a);
        assert_eq!(protocol_text(&reader_b, selection), text_b);
        assert_eq!(shared_a.get_any(selection).unwrap().text().unwrap(), text_a);
        assert_eq!(shared_b.get_any(selection).unwrap().text().unwrap(), text_b);
    }
    drop(shared_a);
    drop(shared_b);
    assert_eq!(
        CLIPBOARDS.lock().len(),
        2,
        "one handle per display still serves native reads"
    );

    let old_a = Arc::downgrade(&clipboard_a.inner);
    let old_b = Arc::downgrade(&clipboard_b.inner);
    let old_a_window = clipboard_a.inner.server.win_id;
    let old_b_window = clipboard_b.inner.server.win_id;
    drop(clipboard_a);
    assert!(
        !CLIPBOARDS.lock().contains_key(a),
        "last A handle removes only A synchronously"
    );
    assert!(CLIPBOARDS.lock().contains_key(b));
    let replacement_a = Clipboard::new(a).expect("recreate A while handover may still run");
    assert!(!Weak::ptr_eq(&old_a, &Arc::downgrade(&replacement_a.inner)));
    drop(clipboard_b);
    assert!(!CLIPBOARDS.lock().contains_key(b));
    let replacement_b = Clipboard::new(b).expect("recreate B while handover may still run");
    assert!(!Weak::ptr_eq(&old_b, &Arc::downgrade(&replacement_b.inner)));

    wait_for_clipboard_handovers();
    assert!(HANDOVERS.lock().is_empty());
    assert!(old_a.upgrade().is_none());
    assert!(old_b.upgrade().is_none());
    {
        let clipboards = CLIPBOARDS.lock();
        assert_eq!(clipboards.len(), 2);
        assert!(Arc::ptr_eq(&clipboards[a].inner, &replacement_a.inner));
        assert!(Arc::ptr_eq(&clipboards[b].inner, &replacement_b.inner));
    }
    for (clipboard, reader, manager, expected, old_window) in [
        (
            &replacement_a,
            &reader_a,
            &manager_a,
            "display A clipboard 文本 🦀",
            old_a_window,
        ),
        (
            &replacement_b,
            &reader_b,
            &manager_b,
            "display B clipboard différent 🦀",
            old_b_window,
        ),
    ] {
        assert!(!clipboard.is_owner(ClipboardKind::Clipboard));
        assert_eq!(
            reader
                .server
                .conn
                .get_selection_owner(reader.atoms.CLIPBOARD)
                .unwrap()
                .reply()
                .unwrap()
                .owner,
            manager.window
        );
        assert_eq!(*manager.saved.lock(), vec![expected.as_bytes().to_vec()]);
        assert_eq!(
            clipboard
                .get_any(ClipboardKind::Clipboard)
                .unwrap()
                .text()
                .unwrap(),
            expected
        );
        assert_eq!(protocol_text(reader, ClipboardKind::Clipboard), expected);
        assert!(
            reader
                .server
                .conn
                .get_window_attributes(old_window)
                .unwrap()
                .reply()
                .is_err()
        );
    }

    let second_a = "display A replacement 新しい payload";
    let second_b = "display B replacement nový payload";
    replacement_a
        .set_text(
            Cow::Borrowed(second_a),
            ClipboardKind::Clipboard,
            WaitConfig::None,
        )
        .unwrap();
    assert_eq!(protocol_text(&reader_a, ClipboardKind::Clipboard), second_a);
    assert_eq!(
        protocol_text(&reader_b, ClipboardKind::Clipboard),
        "display B clipboard différent 🦀"
    );
    replacement_b
        .set_text(
            Cow::Borrowed(second_b),
            ClipboardKind::Clipboard,
            WaitConfig::None,
        )
        .unwrap();
    assert_eq!(protocol_text(&reader_a, ClipboardKind::Clipboard), second_a);
    assert_eq!(protocol_text(&reader_b, ClipboardKind::Clipboard), second_b);

    let new_a = Arc::downgrade(&replacement_a.inner);
    let new_b = Arc::downgrade(&replacement_b.inner);
    let new_a_window = replacement_a.inner.server.win_id;
    let new_b_window = replacement_b.inner.server.win_id;
    drop(replacement_a);
    drop(replacement_b);
    wait_for_clipboard_handovers();
    assert!(HANDOVERS.lock().is_empty());
    assert_released(a, &new_a, &reader_a, new_a_window);
    assert_released(b, &new_b, &reader_b, new_b_window);

    let final_a = Clipboard::new(a).expect("fresh reader A after all old owners joined");
    let final_b = Clipboard::new(b).expect("fresh reader B after all old owners joined");
    assert!(!Weak::ptr_eq(&new_a, &Arc::downgrade(&final_a.inner)));
    assert!(!Weak::ptr_eq(&new_b, &Arc::downgrade(&final_b.inner)));
    assert!(!final_a.is_owner(ClipboardKind::Clipboard));
    assert!(!final_b.is_owner(ClipboardKind::Clipboard));
    assert_eq!(
        final_a
            .get_any(ClipboardKind::Clipboard)
            .unwrap()
            .text()
            .unwrap(),
        second_a
    );
    assert_eq!(
        final_b
            .get_any(ClipboardKind::Clipboard)
            .unwrap()
            .text()
            .unwrap(),
        second_b
    );
    let final_a_owner = Arc::downgrade(&final_a.inner);
    let final_b_owner = Arc::downgrade(&final_b.inner);
    let final_a_window = final_a.inner.server.win_id;
    let final_b_window = final_b.inner.server.win_id;
    drop(final_a);
    drop(final_b);
    wait_for_clipboard_handovers();
    assert!(CLIPBOARDS.lock().is_empty());
    assert!(HANDOVERS.lock().is_empty());
    assert_released(a, &final_a_owner, &reader_a, final_a_window);
    assert_released(b, &final_b_owner, &reader_b, final_b_window);
    assert_eq!(
        manager_a.finish(),
        vec![
            "display A clipboard 文本 🦀".as_bytes().to_vec(),
            second_a.as_bytes().to_vec(),
        ]
    );
    assert_eq!(
        manager_b.finish(),
        vec![
            "display B clipboard différent 🦀".as_bytes().to_vec(),
            second_b.as_bytes().to_vec(),
        ]
    );
}
