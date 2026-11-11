//! Executable acceptance over the host's actual Win32 queue and synchronous renderer.
//!
//! Scripted messages exercise native key decoding, pretranslation and WM_CHAR delivery through
//! GPUI's regular input handler. They establish queue routing, not physical keyboard or IME
//! qualification. Geometry checks query the real HWND and its DPI without claiming a physical
//! monitor or DPI transition.

use super::*;
use anyhow::Context as _;
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::ClientToScreen,
    System::SystemServices::MK_LBUTTON,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC, MapVirtualKeyW, SetFocus, VK_F6},
    },
};

const TEXT_PROBE: &str = "hλ🦀";

#[derive(Clone, Copy)]
enum Phase {
    FirstFrame,
    Idle {
        since: Instant,
        messages: u64,
        blocked: Duration,
    },
    Button {
        after: u64,
    },
    Keyboard {
        after: u64,
    },
    Resize {
        expected: (i32, i32),
        after: u64,
    },
    FinalFrame {
        after: u64,
    },
    Shutdown,
}

pub(super) struct Smoke {
    started: Instant,
    phase: Phase,
    idle_messages: u64,
    idle_blocked: Duration,
    shutdown_prefix: u64,
}

impl Smoke {
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            phase: Phase::FirstFrame,
            idle_messages: 0,
            idle_blocked: Duration::ZERO,
            shutdown_prefix: 0,
        }
    }

    pub(super) fn advance(
        &mut self,
        host: &HostWindow,
        application: &ApplicationHandle,
        factory: &Factory,
        window: Option<&WindowHandle<Root>>,
        input: &RefCell<Option<Entity<HostInput>>>,
        loop_observation: &LoopObservation,
    ) -> Result<()> {
        if self.ready_to_shutdown() {
            return Ok(());
        }
        ensure!(
            self.started.elapsed() < Duration::from_secs(20),
            "external-host smoke timed out during {}",
            self.phase_name()
        );
        let Some(window) = window else {
            return Ok(());
        };
        let submitted = factory.submitted.get();
        let rendered_count = factory.observation.drawn_count.get();
        match self.phase {
            Phase::FirstFrame => {
                if submitted == 0 || host.private_messages.get() == 0 {
                    return Ok(());
                }
                host.verify()?;
                ensure!(
                    host.last_private.get() == 1,
                    "the host's initial private message was corrupted"
                );
                ensure!(rendered_count == 0, "unexpected initial GPU image");
                if !geometry_matches(host, application, window, factory)? {
                    return Ok(());
                }
                self.phase = Phase::Idle {
                    since: Instant::now(),
                    messages: loop_observation.messages,
                    blocked: loop_observation.blocked,
                };
            }
            Phase::Idle {
                since,
                messages,
                blocked,
            } => {
                if since.elapsed() < Duration::from_millis(350) {
                    return Ok(());
                }
                self.idle_messages = loop_observation.messages - messages;
                self.idle_blocked = loop_observation.blocked - blocked;
                ensure!(
                    self.idle_messages <= 512,
                    "unbounded native idle wakes: {} in {:?}",
                    self.idle_messages,
                    since.elapsed()
                );
                ensure!(
                    self.idle_blocked >= Duration::from_millis(100),
                    "the host's GetMessage loop did not block while idle"
                );
                ensure!(
                    host.timer_ticks.get() > 0 && rendered_count == 0,
                    "the host idle/image canary changed unexpectedly"
                );
                click_button(host, application, window, factory)?;
                self.phase = Phase::Button { after: submitted };
            }
            Phase::Button { after } => {
                if submitted <= after || rendered_count != 1 {
                    return Ok(());
                }
                let receiver = input
                    .borrow()
                    .clone()
                    .context("the ordinary text receiver was not created")?;
                application.update(|cx| {
                    window.update(cx, |_, window, cx| {
                        let focus = receiver.read(cx).focus.clone();
                        focus.focus(window, cx);
                        window.refresh();
                    })
                })?;
                // SAFETY: SetFocus enters native callbacks only after the App update returns.
                unsafe {
                    let _ = SetFocus(Some(host.hwnd.get()));
                }
                let scan_code = unsafe { MapVirtualKeyW(VK_F6.0 as u32, MAPVK_VK_TO_VSC) };
                post(
                    host,
                    WM_KEYDOWN,
                    WPARAM(VK_F6.0 as usize),
                    LPARAM(1 | ((scan_code as isize) << 16)),
                )?;
                post(
                    host,
                    WM_KEYUP,
                    WPARAM(VK_F6.0 as usize),
                    LPARAM(1 | ((scan_code as isize) << 16) | (3isize << 30)),
                )?;
                for unit in TEXT_PROBE.encode_utf16() {
                    post(host, WM_CHAR, WPARAM(unit as usize), LPARAM(1))?;
                }
                self.phase = Phase::Keyboard { after: submitted };
            }
            Phase::Keyboard { after } => {
                let receiver = input
                    .borrow()
                    .clone()
                    .context("the text receiver disappeared")?;
                let received = application.update(|cx| {
                    let receiver = receiver.read(cx);
                    receiver.key_events == 1 && receiver.text == TEXT_PROBE
                });
                if submitted <= after || rendered_count != 2 || !received {
                    return Ok(());
                }
                let before = host.client_extent()?;
                let mut outer = RECT::default();
                // SAFETY: Host geometry changes occur outside any active GPUI App borrow.
                unsafe { GetWindowRect(host.hwnd.get(), &mut outer) }?;
                unsafe {
                    SetWindowPos(
                        host.hwnd.get(),
                        None,
                        outer.left + 29,
                        outer.top + 19,
                        outer.right - outer.left + 96,
                        outer.bottom - outer.top + 64,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                }?;
                let expected = host.client_extent()?;
                ensure!(
                    expected.0 > before.0 && expected.1 > before.1,
                    "the real HWND client extent did not change"
                );
                self.phase = Phase::Resize {
                    expected,
                    after: submitted,
                };
            }
            Phase::Resize { expected, after } => {
                if submitted <= after || !geometry_matches(host, application, window, factory)? {
                    return Ok(());
                }
                ensure!(
                    host.client_extent()? == expected,
                    "the host's resized HWND extent changed unexpectedly"
                );
                click_button(host, application, window, factory)?;
                self.phase = Phase::FinalFrame { after: submitted };
            }
            Phase::FinalFrame { after } => {
                if submitted <= after || rendered_count != 3 {
                    return Ok(());
                }
                if factory.observation.completed_prefix() >= submitted {
                    application.update(|cx| window.update(cx, |_, window, _| window.refresh()))?;
                    return Ok(());
                }
                ensure!(
                    factory.observation.retained_recordings.get() > 0,
                    "shutdown requires real retained GPU recordings"
                );
                self.shutdown_prefix = submitted;
                self.phase = Phase::Shutdown;
                println!(
                    "Requesting normal GPUI quit with original GPU prefix {submitted} awaiting completion"
                );
            }
            Phase::Shutdown => {}
        }
        Ok(())
    }

    pub(super) fn ready_to_shutdown(&self) -> bool {
        matches!(self.phase, Phase::Shutdown)
    }

    fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::FirstFrame => "initial submission and host message",
            Phase::Idle { .. } => "bounded native idle",
            Phase::Button { .. } => "native button click",
            Phase::Keyboard { .. } => "native F6/UTF-16 text routing",
            Phase::Resize { .. } => "native/GPUI/renderer geometry agreement",
            Phase::FinalFrame { .. } => "final image submission",
            Phase::Shutdown => "shutdown",
        }
    }

    pub(super) fn verify_detached(&self, factory: &Factory) -> Result<()> {
        ensure!(
            self.ready_to_shutdown(),
            "the host closed before smoke acceptance completed"
        );
        let observation = &factory.observation;
        let ticket = observation.shutdown_ticket.borrow();
        let snapshot = ticket
            .as_ref()
            .context("the actual renderer never received its shutdown ticket")?
            .snapshot();
        ensure!(
            snapshot.quiesced() && snapshot.detached() && snapshot.binding_terminal(),
            "the renderer's real ticket did not acknowledge quiesced detachment: {snapshot:?}"
        );
        ensure!(
            !snapshot.native_terminal() && !snapshot.protocol_violation(),
            "host detachment was reported as OS destruction: {snapshot:?}"
        );
        let quiesced = observation
            .quiesced_prefix
            .get()
            .context("the original renderer did not quiesce")?;
        ensure!(
            quiesced >= self.shutdown_prefix && observation.completed_prefix() >= quiesced,
            "shutdown did not finish the original submitted GPU work"
        );
        ensure!(
            observation.retained_recordings.get() == 0,
            "shutdown retained GPU recordings"
        );
        Ok(())
    }

    pub(super) fn report(&self, factory: &Factory, observation: &LoopObservation) {
        println!(
            "External-host smoke: changed GPU images, native button/F6/UTF-16 routing, native/GPUI/render resize, \
             idle {} messages / {:?} blocked; quiesced prefix {}; HWND enabled, userdata/private messages preserved, \
             typed Detached without OS destruction; host WM_QUIT probe observed: {}",
            self.idle_messages,
            self.idle_blocked,
            factory.observation.quiesced_prefix.get().unwrap_or(0),
            observation.quit_probe_seen,
        );
    }
}

fn post(host: &HostWindow, message: u32, wparam: WPARAM, lparam: LPARAM) -> Result<()> {
    // SAFETY: These native input packets carry no pointers and target this host's live HWND.
    unsafe { PostMessageW(Some(host.hwnd.get()), message, wparam, lparam) }?;
    Ok(())
}

fn click_button(
    host: &HostWindow,
    application: &ApplicationHandle,
    window: &WindowHandle<Root>,
    factory: &Factory,
) -> Result<()> {
    let bounds = factory
        .observation
        .button_bounds
        .get()
        .context("the real button was not laid out")?;
    let scale = application.update(|cx| window.update(cx, |_, window, _| window.scale_factor()))?;
    let point = bounds.to_device_pixels(scale).center();
    let x = point.x.0;
    let y = point.y.0;
    ensure!(
        x >= 0 && y >= 0 && x <= i16::MAX as i32 && y <= i16::MAX as i32,
        "the button is outside native mouse coordinate range"
    );
    let position = LPARAM((x as u16 as isize) | ((y as u16 as isize) << 16));
    post(host, WM_MOUSEMOVE, WPARAM(0), position)?;
    post(
        host,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        position,
    )?;
    post(host, WM_LBUTTONUP, WPARAM(0), position)?;
    Ok(())
}

fn geometry_matches(
    host: &HostWindow,
    application: &ApplicationHandle,
    window: &WindowHandle<Root>,
    factory: &Factory,
) -> Result<bool> {
    let native_extent = host.client_extent()?;
    let mut origin = POINT::default();
    // SAFETY: Native client-to-screen conversion uses writable local storage.
    ensure!(
        unsafe { ClientToScreen(host.hwnd.get(), &mut origin) }.as_bool(),
        "reading the HWND's actual client origin"
    );
    let (bounds, viewport, scale) = application.update(|cx| {
        window.update(cx, |_, window, _| {
            let scale = window.scale_factor();
            (
                window.bounds().to_device_pixels(scale),
                window.viewport_size().to_device_pixels(scale),
                scale,
            )
        })
    })?;
    let dpi = unsafe { GetDpiForWindow(host.hwnd.get()) };
    ensure!(
        dpi > 0 && (scale - dpi as f32 / 96.).abs() < 0.01,
        "GPUI scale disagrees with the queried HWND DPI"
    );
    Ok((bounds.origin.x.0 - origin.x).abs() <= 1
        && (bounds.origin.y.0 - origin.y).abs() <= 1
        && (bounds.size.width.0, bounds.size.height.0) == native_extent
        && (viewport.width.0, viewport.height.0) == native_extent
        && factory.observation.extent.get() == native_extent
        && factory.observation.drawn_extent.get() == native_extent)
}
