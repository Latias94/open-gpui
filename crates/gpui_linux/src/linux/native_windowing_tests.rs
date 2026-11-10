use super::*;
use anyhow::Context as _;
use open_gpui::{
    AppContext as _, Application, AsyncApp, Context, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, QuitMode, Render, Styled as _, Window, WindowBounds,
    WindowHandle, WindowOptions, canvas, div, fill, point, px, rgb, size,
};
use std::{
    cell::Cell,
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const CHILD: &str = "GPUI_LINUX_NATIVE_WINDOWING_SCENARIO";
const EXEC_SOCKET: &str = "GPUI_LINUX_NATIVE_WINDOWING_SOCKET_IDENTITY";
const MISSING_STARTUP_DISPLAY: &str = "/gpui-test/missing-owned-startup-socket";
const INITIAL_COLOR: u32 = 0x315ea8;
const INPUT_COLOR: u32 = 0xb85c24;

#[derive(Clone, Copy, Debug, Default)]
struct PaintStamp {
    version: usize,
    generation: u64,
    color: u32,
}

struct Painted {
    render_count: Rc<Cell<usize>>,
    focus: FocusHandle,
    version: usize,
    paint: Rc<Cell<PaintStamp>>,
}

impl Painted {
    fn new(render_count: Rc<Cell<usize>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            render_count,
            focus,
            version: 0,
            paint: Rc::new(Cell::new(PaintStamp::default())),
        }
    }
}

impl Render for Painted {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_count.set(self.render_count.get() + 1);
        let version = self.version;
        let color = if version == 0 {
            INITIAL_COLOR
        } else {
            INPUT_COLOR
        };
        let paint = self.paint.clone();
        div()
            .size_full()
            .track_focus(&self.focus)
            .debug_selector(move || format!("native-windowing-{version}-{color:06x}"))
            .on_key_down(cx.listener(|view, event: &open_gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "f8" {
                    view.version += 1;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        window.paint_quad(fill(bounds, rgb(color)));
                        paint.set(PaintStamp {
                            version,
                            generation: window.rendered_frame_revision().saturating_add(1),
                            color,
                        });
                    },
                )
                .size_full(),
            )
    }
}

/// Creates and retires real hidden X11 and Wayland windows on one App and native loop.
/// Set GPUI_LINUX_TEST_WAYLAND_DISPLAY to an absolute socket path when XDG_RUNTIME_DIR does not
/// name the compositor's socket directory, as with some WSLg installations.
#[test]
#[ignore = "requires live X11, Wayland and a working native WGPU renderer"]
fn native_hidden_windowing_roundtrip() {
    run_probe("native_hidden_windowing_roundtrip", false);
}

/// Additionally requires a nonempty Submitted frame from each native renderer. This briefly maps
/// one small window at a time; the hidden probe does not claim compositor presentation.
#[test]
#[ignore = "briefly maps native windows and requires live X11, Wayland and WGPU"]
fn native_presented_windowing_roundtrip() {
    run_probe("native_presented_windowing_roundtrip", true);
}

fn is_probe_child(name: &str, marker: &str) -> bool {
    if env::var(CHILD).as_deref() != Ok(name) {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!("{module}::{name}");
        #[allow(
            clippy::disallowed_methods,
            reason = "the probe runs in an isolated child with a bounded lifetime"
        )]
        let mut child = Command::new(env::current_exe().unwrap())
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env(CHILD, name)
            .env_remove("WAYLAND_SOCKET")
            .env_remove("XDG_ACTIVATION_TOKEN")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start native windowing probe");
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().ok();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "native windowing probe timed out\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "native probe failed: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(marker));
        for line in stdout.lines().filter(|line| line.starts_with("NATIVE_")) {
            println!("{line}");
        }
        return false;
    }
    true
}

fn run_probe(name: &str, show: bool) {
    if !is_probe_child(name, "NATIVE_WINDOWING_ROUNDTRIP_COMPLETE") {
        return;
    }

    let platform = crate::linux::linux_platform_with_startup(WindowingModes::all(), None, None);
    let completed = Rc::new(Cell::new(false));
    let outcome = Rc::new(RefCell::new(None));
    Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .with_windowing(WindowingRequest::Headless)
        .run({
            let completed = completed.clone();
            let outcome = outcome.clone();
            move |cx| {
                assert!(cx.displays().is_empty());
                assert!(cx.graphical_environment().is_none());
                cx.spawn(async move |cx| {
                    *outcome.borrow_mut() = Some(roundtrip(cx, show, platform).await);
                    completed.set(true);
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
    assert!(
        completed.get(),
        "the native loop exited without completing its lifecycle probe"
    );
    outcome
        .borrow_mut()
        .take()
        .unwrap()
        .expect("native lifecycle roundtrip");
    println!("NATIVE_WINDOWING_ROUNDTRIP_COMPLETE show={show}");
}

async fn roundtrip(
    cx: &mut AsyncApp,
    show: bool,
    platform: Rc<dyn Platform>,
) -> anyhow::Result<()> {
    let detected = GraphicalEnvironment::detect();
    let x11_display = env::var_os("GPUI_LINUX_TEST_X11_DISPLAY")
        .or(detected.x11_display.clone())
        .context("supply DISPLAY or GPUI_LINUX_TEST_X11_DISPLAY")?;
    let wayland_display = env::var_os("GPUI_LINUX_TEST_WAYLAND_DISPLAY")
        .or(detected.wayland_display.clone())
        .context("supply WAYLAND_DISPLAY or GPUI_LINUX_TEST_WAYLAND_DISPLAY")?;
    let x11 = GraphicalEnvironment {
        x11_display: Some(x11_display),
        xdg_runtime_dir: detected.xdg_runtime_dir.clone(),
        ..GraphicalEnvironment::default()
    };
    let wayland = GraphicalEnvironment {
        wayland_display: Some(wayland_display),
        xdg_runtime_dir: detected.xdg_runtime_dir,
        ..GraphicalEnvironment::default()
    };
    let entity = cx.new(|_| 0_usize);
    let text_system = cx.update(|cx| cx.text_system().clone());
    let foreground = Rc::new(Cell::new(0));
    let render_count = Rc::new(Cell::new(0));

    for (backend, environment) in [("X11", x11), ("Wayland", wayland)] {
        let validations = Rc::new(Cell::new(0));
        let refused = platform
            .request_windowing_with_validation(
                WindowingRequest::Windowed(environment.clone()),
                Box::new({
                    let validations = validations.clone();
                    move || {
                        validations.set(validations.get() + 1);
                        anyhow::ensure!(validations.get() < 3, "late native owner refusal");
                        Ok(())
                    }
                }),
            )
            .await;
        assert_eq!(
            validations.get(),
            3,
            "revalidate immediately before the physical replace"
        );
        assert!(
            refused
                .unwrap_err()
                .to_string()
                .contains("late native owner refusal")
        );
        assert_eq!(platform.compositor_name(), "headless");
        assert!(platform.displays().is_empty());
        cx.update(|cx| cx.request_windowing(WindowingRequest::Windowed(environment.clone())))
            .await
            .with_context(|| format!("attach {backend}"))?;
        cx.update(|cx| {
            assert_eq!(cx.compositor_name(), backend);
            assert!(
                !cx.displays().is_empty(),
                "displays must be fresh before opening the first window"
            );
            assert!(Arc::ptr_eq(&text_system, cx.text_system()));
            let connected = cx.graphical_environment().unwrap();
            assert_eq!(connected.wayland_display, environment.wayland_display);
            assert_eq!(connected.x11_display, environment.x11_display);
            assert_eq!(connected.xdg_runtime_dir, environment.xdg_runtime_dir);
            assert!(connected.activation_token.is_none());
        });
        entity.update(cx, |value, _| *value += 1);
        assert_eq!(cx.background_executor().spawn(async { 42 }).await, 42);
        cx.spawn({
            let foreground = foreground.clone();
            async move |_| foreground.set(foreground.get() + 1)
        })
        .await;

        let window = cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    show,
                    window_bounds: Some(WindowBounds::Windowed(open_gpui::Bounds::new(
                        point(px(30.0), px(30.0)),
                        size(px(160.0), px(100.0)),
                    ))),
                    ..WindowOptions::default()
                },
                |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
            )
        })?;
        let refusal = cx
            .update(|cx| cx.request_windowing(WindowingRequest::Headless))
            .await;
        let gpu = cx
            .update(|cx| window.update(cx, |_, window, _| window.gpu_specs()))?
            .context("the native window must expose its actual GPU")?;
        println!("NATIVE_WINDOWING_GPU {backend}: {gpu:?}");
        assert!(
            refusal
                .unwrap_err()
                .to_string()
                .contains("windows are open")
        );

        if show {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let facts =
                    cx.update(|cx| window.update(cx, |_, window, _| window.presentation_facts()))?;
                if facts.non_empty_presented_generation.is_some() {
                    assert!(facts.present_submitted_generation.is_some());
                    assert_eq!(
                        facts.latest_present_attempt.unwrap().outcome,
                        open_gpui::PlatformWindowPresentOutcome::Submitted
                    );
                    break;
                }
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "{backend} never submitted a nonempty native frame: {facts:?}"
                );
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            }
            if backend == "X11" {
                exercise_x11_keyboard(cx, window, &environment).await?;
            }
        } else {
            cx.update_window(window.into(), |_, window, cx| {
                let clear = window.draw(cx);
                clear.clear(cx);
                assert!(
                    window
                        .presentation_facts()
                        .frame_accepted_generation
                        .is_some()
                );
                assert!(!window.presentation_facts().native_visible);
                assert!(window.gpu_specs().is_some());
            })?;
        }

        cx.update(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match cx
                .update(|cx| cx.request_windowing(WindowingRequest::Headless))
                .await
            {
                Ok(()) => break,
                Err(error) => {
                    let message = error.to_string();
                    anyhow::ensure!(
                        message.contains("retiring") || message.contains("windows are open"),
                        "unexpected detach refusal from {backend}: {error:#}"
                    );
                    anyhow::ensure!(
                        Instant::now() < deadline,
                        "{backend} native retirement never completed: {error:#}"
                    );
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                }
            }
        }
        cx.update(|cx| {
            assert_eq!(cx.compositor_name(), "headless");
            assert!(cx.graphical_environment().is_none());
            assert!(cx.displays().is_empty());
            assert!(cx.windows().is_empty());
            assert!(Arc::ptr_eq(&text_system, cx.text_system()));
        });
        entity.update(cx, |value, _| *value += 1);
    }
    assert_eq!(entity.read_with(cx, |value, _| *value), 4);
    assert_eq!(foreground.get(), 2);
    assert!(render_count.get() >= 2);
    Ok(())
}

async fn exercise_x11_keyboard(
    cx: &mut AsyncApp,
    handle: WindowHandle<Painted>,
    environment: &GraphicalEnvironment,
) -> anyhow::Result<()> {
    use raw_window_handle::RawWindowHandle;
    use x11rb::{
        connection::Connection as _,
        protocol::xproto::{
            ConnectionExt as _, EventMask, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, KeyButMask,
            KeyPressEvent,
        },
    };

    let display = environment.x11_display.as_ref().unwrap().to_string_lossy();
    let (connection, screen) = x11rb::connect(Some(&display))?;
    let setup = connection.setup();
    let mapping = connection
        .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)?
        .reply()?;
    anyhow::ensure!(
        mapping.keysyms_per_keycode > 0,
        "X11 returned an empty keyboard mapping"
    );
    let keycode = mapping
        .keysyms
        .chunks(usize::from(mapping.keysyms_per_keycode))
        .position(|keysyms| keysyms.contains(&0xffc5))
        .context("the native X11 keymap must expose F8")?;
    let keycode = setup
        .min_keycode
        .checked_add(u8::try_from(keycode)?)
        .context("F8 keycode must fit the X11 protocol")?;
    let (native_window, previous_generation, previous_keys) = cx.update(|cx| {
        handle.update(cx, |view, window, cx| {
            let RawWindowHandle::Xcb(raw) =
                raw_window_handle::HasWindowHandle::window_handle(window)?.as_raw()
            else {
                anyhow::bail!("the X11 window did not expose an Xcb handle");
            };
            assert_eq!(view.version, 0);
            assert_eq!(view.paint.get().color, INITIAL_COLOR);
            assert!(view.focus.is_focused(window));
            assert!(window.focused(cx).is_some());
            Ok((
                raw.window.get(),
                window.rendered_frame_revision(),
                window.key_event_revision(),
            ))
        })
    })??;
    let press = KeyPressEvent {
        response_type: KEY_PRESS_EVENT,
        detail: keycode,
        sequence: 0,
        time: x11rb::CURRENT_TIME,
        root: setup.roots[screen].root,
        event: native_window,
        child: x11rb::NONE,
        root_x: 0,
        root_y: 0,
        event_x: 8,
        event_y: 8,
        state: KeyButMask::default(),
        same_screen: true,
    };
    connection
        .send_event(false, native_window, EventMask::KEY_PRESS, press)?
        .check()?;
    connection
        .send_event(
            false,
            native_window,
            EventMask::KEY_RELEASE,
            KeyPressEvent {
                response_type: KEY_RELEASE_EVENT,
                ..press
            },
        )?
        .check()?;
    connection.flush()?;

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (version, stamp, facts, key_revision, submitted_version) = cx.update(|cx| {
            handle.update(cx, |view, window, _| {
                let stamp = view.paint.get();
                let submitted_version =
                    window
                        .submitted_frame_diagnostics_for_test()
                        .is_some_and(|diagnostics| {
                            diagnostics.generation() == stamp.generation
                                && diagnostics
                                    .debug_bounds()
                                    .iter()
                                    .any(|(selector, _)| selector == "native-windowing-1-b85c24")
                        });
                (
                    view.version,
                    stamp,
                    window.presentation_facts(),
                    window.key_event_revision(),
                    submitted_version,
                )
            })
        })?;
        anyhow::ensure!(
            version <= 1,
            "one native F8 press changed the retained model more than once"
        );
        if version == 1
            && stamp.version == 1
            && stamp.color == INPUT_COLOR
            && stamp.generation > previous_generation
            && key_revision >= previous_keys + 2
            && submitted_version
            && facts.frame_accepted_generation == Some(stamp.generation)
            && facts.non_empty_presented_generation == Some(stamp.generation)
            && facts.present_submitted_generation == Some(stamp.generation)
            && facts.latest_present_attempt.is_some_and(|attempt| {
                attempt.generation == stamp.generation
                    && attempt.contained_valid_primitives
                    && attempt.outcome == open_gpui::PlatformWindowPresentOutcome::Submitted
            })
        {
            break;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "native X11 F8 input never reached a new nonempty Submitted color version: version={version}, paint={stamp:?}, facts={facts:?}, keys={key_revision}"
        );
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
    }
    println!("NATIVE_X11_KEYBOARD_SUBMITTED version=1 color={INPUT_COLOR:06x}");
    Ok(())
}

/// Transfers a real compositor socket, checks its identity across exec and retirement, then
/// reconnects by address. Also proves that an initially headless App discards the one-shot socket.
#[test]
#[ignore = "briefly maps Wayland windows and requires a live compositor and native WGPU"]
fn native_owned_startup_socket_is_initial_only_and_close_on_exec() {
    if let Ok(identity) = env::var(EXEC_SOCKET) {
        let (device, inode) = identity.split_once(':').expect("socket device:inode");
        let identity = (device.parse().unwrap(), inode.parse().unwrap());
        assert!(
            !has_socket_identity(identity),
            "the compositor connection survived exec"
        );
        println!("NATIVE_STARTUP_SOCKET_EXEC_COMPLETE");
        return;
    }
    let name = "native_owned_startup_socket_is_initial_only_and_close_on_exec";
    if !is_probe_child(name, "NATIVE_OWNED_STARTUP_SOCKET_COMPLETE") {
        return;
    }
    let detected = GraphicalEnvironment::detect();
    let display = env::var_os("GPUI_LINUX_TEST_WAYLAND_DISPLAY")
        .or(detected.wayland_display)
        .expect("supply WAYLAND_DISPLAY or GPUI_LINUX_TEST_WAYLAND_DISPLAY");
    let address = PathBuf::from(&display);
    let address = if address.is_absolute() {
        address
    } else {
        PathBuf::from(
            detected
                .xdg_runtime_dir
                .as_ref()
                .expect("relative Wayland display requires XDG_RUNTIME_DIR"),
        )
        .join(address)
    };
    let environment = GraphicalEnvironment {
        wayland_display: Some(display),
        xdg_runtime_dir: detected.xdg_runtime_dir,
        ..GraphicalEnvironment::default()
    };
    for start_headless in [false, true] {
        let socket = UnixStream::connect(&address)
            .expect("connect an owned startup socket to the compositor");
        let identity = socket_identity(socket.as_raw_fd()).expect("stat the original owned socket");
        // SAFETY: This stream exclusively owns the live descriptor. Clearing its close-on-exec
        // flag models an inherited startup socket before ownership passes to the platform.
        let flags = unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_GETFD) };
        assert_ne!(flags, -1);
        // SAFETY: No child is spawned until the platform has taken this owned descriptor.
        assert_ne!(
            unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            -1
        );
        let platform =
            crate::linux::linux_platform_with_startup(WindowingModes::all(), Some(socket), None);
        assert!(has_socket_identity(identity));
        assert_socket_not_inherited(name, identity);
        let weak_platform = Rc::downgrade(&platform);
        let completed = Rc::new(Cell::new(false));
        let outcome = Rc::new(RefCell::new(None));
        let initial = if start_headless {
            WindowingRequest::Headless
        } else {
            assert!(UnixStream::connect(MISSING_STARTUP_DISPLAY).is_err());
            WindowingRequest::Windowed(GraphicalEnvironment {
                wayland_display: Some(MISSING_STARTUP_DISPLAY.into()),
                ..environment.clone()
            })
        };
        Application::with_platform(platform.clone())
            .with_quit_mode(QuitMode::Explicit)
            .with_windowing(initial)
            .run({
                let completed = completed.clone();
                let outcome = outcome.clone();
                let environment = environment.clone();
                move |cx| {
                    assert_eq!(has_socket_identity(identity), !start_headless);
                    assert_socket_not_inherited(name, identity);
                    cx.spawn(async move |cx| {
                        *outcome.borrow_mut() = Some(
                            owned_startup_roundtrip(cx, environment, identity, start_headless)
                                .await,
                        );
                        completed.set(true);
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
            });
        assert!(
            completed.get(),
            "the native loop exited before the owned socket probe finished"
        );
        outcome
            .borrow_mut()
            .take()
            .unwrap()
            .expect("owned startup socket lifecycle");
        assert!(!has_socket_identity(identity));
        drop(platform);
        assert!(
            weak_platform.upgrade().is_none(),
            "the closed runtime retained its platform"
        );
    }
    println!("NATIVE_OWNED_STARTUP_SOCKET_COMPLETE");
}

fn socket_identity(fd: libc::c_int) -> std::io::Result<(u64, u64)> {
    let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes a complete stat on success; failure leaves it unread.
    if unsafe { libc::fstat(fd, status.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fstat succeeded and initialized the complete structure above.
    let status = unsafe { status.assume_init() };
    Ok((status.st_dev as u64, status.st_ino as u64))
}

fn has_socket_identity(identity: (u64, u64)) -> bool {
    std::fs::read_dir("/proc/self/fd")
        .expect("enumerate this native probe's descriptors")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .parse::<libc::c_int>()
                .ok()
        })
        .any(|fd| socket_identity(fd).ok() == Some(identity))
}

fn assert_socket_not_inherited(name: &str, identity: (u64, u64)) {
    let module = module_path!().split_once("::").unwrap().1;
    let test = format!("{module}::{name}");
    #[allow(
        clippy::disallowed_methods,
        reason = "the exec probe is isolated and has a bounded lifetime"
    )]
    let mut child = Command::new(env::current_exe().unwrap())
        .args(["--exact", &test, "--ignored", "--nocapture"])
        .env(EXEC_SOCKET, format!("{}:{}", identity.0, identity.1))
        .env_remove("WAYLAND_SOCKET")
        .env_remove("XDG_ACTIVATION_TOKEN")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("exec the descriptor identity probe");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().ok();
            let output = child.wait_with_output().unwrap();
            panic!(
                "startup socket exec probe timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .contains("NATIVE_STARTUP_SOCKET_EXEC_COMPLETE"),
        "startup socket exec probe failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn owned_startup_roundtrip(
    cx: &mut AsyncApp,
    environment: GraphicalEnvironment,
    identity: (u64, u64),
    start_headless: bool,
) -> anyhow::Result<()> {
    let entity = cx.new(|_| 0_usize);
    let entity_id = entity.entity_id();
    let text_system = cx.update(|cx| cx.text_system().clone());
    let render_count = Rc::new(Cell::new(0));
    let foreground = Rc::new(Cell::new(0));
    for reconnect in 0..if start_headless { 1 } else { 2 } {
        if start_headless || reconnect > 0 {
            let mut missing = environment.clone();
            missing.wayland_display = Some(MISSING_STARTUP_DISPLAY.into());
            let error = cx
                .update(|cx| cx.request_windowing(WindowingRequest::Windowed(missing)))
                .await
                .expect_err("a consumed startup socket must not rescue an unreachable new address");
            assert!(format!("{error:#}").contains("failed to connect"));
            assert!(!has_socket_identity(identity));
            cx.update(|cx| {
                assert_eq!(cx.compositor_name(), "headless");
                assert!(cx.graphical_environment().is_none());
                assert!(cx.displays().is_empty());
            });
            cx.update(|cx| cx.request_windowing(WindowingRequest::Windowed(environment.clone())))
                .await
                .context("reconnect with the explicit Wayland address")?;
        }
        cx.update(|cx| {
            assert_eq!(cx.compositor_name(), "Wayland");
            assert!(!cx.displays().is_empty());
            assert!(Arc::ptr_eq(&text_system, cx.text_system()));
            let actual = cx.graphical_environment().unwrap();
            let expected_display = if !start_headless && reconnect == 0 {
                Some(MISSING_STARTUP_DISPLAY.into())
            } else {
                environment.wayland_display.clone()
            };
            assert_eq!(actual.wayland_display, expected_display);
            assert_eq!(actual.xdg_runtime_dir, environment.xdg_runtime_dir);
            assert!(actual.activation_token.is_none());
        });
        assert_eq!(entity.entity_id(), entity_id);
        assert_eq!(cx.background_executor().spawn(async { 42 }).await, 42);
        cx.spawn({
            let foreground = foreground.clone();
            async move |_| foreground.set(foreground.get() + 1)
        })
        .await;
        let handle = cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    show: true,
                    window_bounds: Some(WindowBounds::Windowed(open_gpui::Bounds::new(
                        point(px(30.0), px(30.0)),
                        size(px(160.0), px(100.0)),
                    ))),
                    ..WindowOptions::default()
                },
                |window, cx| cx.new(|cx| Painted::new(render_count.clone(), window, cx)),
            )
        })?;
        assert!(
            cx.update(|cx| cx.request_windowing(WindowingRequest::Headless))
                .await
                .unwrap_err()
                .to_string()
                .contains("windows are open")
        );
        let gpu = cx
            .update(|cx| handle.update(cx, |_, window, _| window.gpu_specs()))?
            .context("the owned-socket window must expose its actual GPU")?;
        println!("NATIVE_OWNED_SOCKET_GPU: {gpu:?}");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (stamp, facts, submitted_version) = cx.update(|cx| {
                handle.update(cx, |view, window, _| {
                    let stamp = view.paint.get();
                    let submitted_version = window
                        .submitted_frame_diagnostics_for_test()
                        .is_some_and(|diagnostics| {
                            diagnostics.generation() == stamp.generation
                                && diagnostics
                                    .debug_bounds()
                                    .iter()
                                    .any(|(selector, _)| selector == "native-windowing-0-315ea8")
                        });
                    (stamp, window.presentation_facts(), submitted_version)
                })
            })?;
            if stamp.generation > 0
                && stamp.color == INITIAL_COLOR
                && submitted_version
                && facts.frame_accepted_generation == Some(stamp.generation)
                && facts.non_empty_presented_generation == Some(stamp.generation)
                && facts.present_submitted_generation == Some(stamp.generation)
                && facts.latest_present_attempt.is_some_and(|attempt| {
                    attempt.generation == stamp.generation
                        && attempt.contained_valid_primitives
                        && attempt.outcome == open_gpui::PlatformWindowPresentOutcome::Submitted
                })
            {
                break;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "owned startup/reconnect never submitted a nonempty frame: {stamp:?}, {facts:?}"
            );
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
        entity.update(cx, |value, _| *value += 1);
        cx.update(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match cx
                .update(|cx| cx.request_windowing(WindowingRequest::Headless))
                .await
            {
                Ok(()) => break,
                Err(error) => {
                    let message = format!("{error:#}");
                    anyhow::ensure!(
                        message.contains("retiring") || message.contains("windows are open"),
                        "unexpected owned-socket detach refusal: {error:#}"
                    );
                    anyhow::ensure!(
                        Instant::now() < deadline,
                        "owned-socket native retirement never finished: {error:#}"
                    );
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                }
            }
        }
        assert!(
            !has_socket_identity(identity),
            "the retired connection retained the startup socket"
        );
        cx.update(|cx| {
            assert_eq!(cx.compositor_name(), "headless");
            assert!(cx.graphical_environment().is_none());
            assert!(cx.displays().is_empty());
            assert!(cx.windows().is_empty());
            assert!(Arc::ptr_eq(&text_system, cx.text_system()));
        });
    }
    let expected = if start_headless { 1 } else { 2 };
    assert_eq!(entity.read_with(cx, |value, _| *value), expected);
    assert_eq!(foreground.get(), expected);
    assert!(render_count.get() >= expected);
    Ok(())
}
