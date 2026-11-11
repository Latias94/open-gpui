use super::*;
use open_gpui::PlatformFrameSubmissionStatus;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::Instant;

#[derive(Clone)]
struct Observation {
    prefix: u64,
    offers: u64,
    packet_wakes: u64,
    wake_turns: u64,
    empty_drives: u64,
    pending: bool,
    needs_frame: bool,
    token: PlatformFrameSubmission,
    output: Rc<renderer::Output>,
}

impl Observation {
    fn stamp(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.prefix,
            self.offers,
            self.packet_wakes,
            self.wake_turns,
            self.empty_drives,
        )
    }
}

pub(crate) fn start(
    cx: &mut App,
    logical: Rc<RefCell<Option<LogicalRoot>>>,
    factory: Rc<Factory>,
    native: WindowHandle<Root>,
    native_terminal: Rc<Cell<bool>>,
) {
    let logical_id = logical.borrow().as_ref().unwrap().window.window_id();
    let close_requested = Rc::new(Cell::new(false));
    let isolated = Rc::new(Cell::new(false));
    let verified = Rc::new(Cell::new(false));
    cx.on_window_native_terminal({
        let logical = logical.clone();
        let factory = factory.clone();
        let close_requested = close_requested.clone();
        let isolated = isolated.clone();
        let verified = verified.clone();
        move |_, terminal_id| {
            if terminal_id != logical_id {
                return;
            }
            let result = (|| -> Result<()> {
                ensure!(
                    close_requested.get() && isolated.get() && native_terminal.get(),
                    "logical retirement did not follow isolated demand and the original native terminal"
                );
                let owner = logical.borrow();
                let root = owner.as_ref().context("native close lost the logical owner")?;
                root.verify_shutdown(false)?;
                let native_ticket = factory.observation.shutdown_ticket.borrow();
                let native_snapshot = native_ticket
                    .as_ref()
                    .context("normal native close lost its renderer shutdown ticket")?
                    .snapshot();
                ensure!(
                    native_snapshot.window_id() == native.window_id()
                        && native_snapshot.quiesced()
                        && native_snapshot.native_terminal()
                        && !native_snapshot.detached()
                        && !native_snapshot.protocol_violation(),
                    "normal native close did not publish exact quiesced OS destruction: {native_snapshot:?}"
                );
                ensure!(
                    factory.gpu.errors.load(Ordering::Relaxed) == 0
                        && factory.observation.failures.get() == 0,
                    "native-close smoke reported a GPU or UI error"
                );
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("Normal native-close smoke failed: {error:#}");
                std::process::exit(1);
            }
            verified.set(true);
            println!(
                "Normal owned native-close smoke: live logical root survived until the original HWND's WM_CLOSE; \
                exact native terminal requested normal App quit; original logical GPU prefix quiesced and detached"
            );
        }
    })
    .detach();
    cx.spawn(async move |cx| {
        let result = run(
            cx,
            &logical,
            &factory,
            &native,
            &close_requested,
            &isolated,
            &verified,
        )
        .await;
        if let Err(error) = result {
            eprintln!("Normal native-close smoke failed: {error:#}");
            std::process::exit(1);
        }
    })
    .detach();
}

async fn run(
    cx: &AsyncApp,
    logical: &RefCell<Option<LogicalRoot>>,
    factory: &Factory,
    native: &WindowHandle<Root>,
    close_requested: &Cell<bool>,
    isolated: &Cell<bool>,
    verified: &Cell<bool>,
) -> Result<()> {
    let started = Instant::now();
    let mut quiet: Option<(Observation, Instant)> = None;
    let mut demand: Option<Observation> = None;
    let mut closing = None;
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        if verified.get() {
            return Ok(());
        }
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "isolated-demand/native-close smoke timed out"
        );
        if let Some(since) = closing {
            ensure!(
                Instant::now().duration_since(since) < Duration::from_secs(10),
                "normal native close did not retire the live logical root"
            );
            continue;
        }
        let owner = logical.borrow();
        let root = owner
            .as_ref()
            .context("isolated demand lost its logical owner")?;
        let Some(current) = observe(root)? else {
            continue;
        };
        if let Some(before) = demand.as_ref() {
            if current.prefix == before.prefix {
                continue;
            }
            ensure!(
                current.prefix > before.prefix
                    && current.packet_wakes > before.packet_wakes
                    && current.wake_turns >= before.wake_turns + 2
                    && current.empty_drives > before.empty_drives
                    && current.token.status() == PlatformFrameSubmissionStatus::Submitted
                    && current.token.window_id() == root.window.window_id()
                    && current.token.attempt_id() != before.token.attempt_id()
                    && !Rc::ptr_eq(&current.output, &before.output)
                    && current.output.extent == before.output.extent,
                "isolated demand did not cross async draw and a later host wake into an original GPU submission"
            );
            let facts = cx.update(|cx| {
                root.window
                    .update(cx, |_, window, _| window.presentation_facts())
            })?;
            if facts.present_submitted_generation != Some(current.token.frame_generation()) {
                continue;
            }
            ensure!(
                !root.surface.is_closed()
                    && !root.renderer.delivery.borrow().quiesced
                    && factory.submitted.get() > 0
                    && cx.update(|cx| cx.windows().len()) == 2,
                "normal native close requires its still-live logical sibling and original native output"
            );
            isolated.set(true);
            println!(
                "Isolated owned logical demand: prefix {} -> {}, host wake turns {} -> {}, \
                packet wakes {} -> {}, async empty drives {} -> {}; timer only observed, no periodic process_pending",
                before.prefix,
                current.prefix,
                before.wake_turns,
                current.wake_turns,
                before.packet_wakes,
                current.packet_wakes,
                before.empty_drives,
                current.empty_drives,
            );
            close_requested.set(true);
            post_native_close(cx, native)?;
            closing = Some(Instant::now());
            continue;
        }
        if current.pending || current.needs_frame {
            quiet = None;
            continue;
        }
        if quiet
            .as_ref()
            .is_none_or(|(previous, _)| previous.stamp() != current.stamp())
        {
            quiet = Some((current, Instant::now()));
            continue;
        }
        if quiet.as_ref().unwrap().1.elapsed() >= Duration::from_millis(120) {
            demand = Some(current);
            root.request_frame()?;
        }
    }
}

fn observe(root: &LogicalRoot) -> Result<Option<Observation>> {
    let delivery = root.renderer.delivery.borrow();
    ensure!(
        delivery.failure.is_none(),
        "isolated renderer failed: {:?}",
        delivery.failure
    );
    let Some(last) = delivery.last_output.as_ref() else {
        return Ok(None);
    };
    Ok(Some(Observation {
        prefix: delivery.prefix,
        offers: delivery.offers,
        packet_wakes: delivery.packet_wakes,
        wake_turns: root.wake_turns.get(),
        empty_drives: root.empty_drives.get(),
        pending: delivery.pending.is_some(),
        needs_frame: root.surface.needs_frame(),
        token: last.token.clone(),
        output: last.output.clone(),
    }))
}

fn post_native_close(cx: &AsyncApp, native: &WindowHandle<Root>) -> Result<()> {
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE},
    };
    let hwnd = cx.update(|cx| {
        native.update(cx, |_, window, _| {
            let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)?.as_raw()
            else {
                anyhow::bail!("native-close smoke lost its original HWND");
            };
            Ok::<_, anyhow::Error>(HWND(handle.hwnd.get() as *mut std::ffi::c_void))
        })
    })??;
    // SAFETY: WM_CLOSE targets the original live owned HWND after the App borrow has ended.
    unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }?;
    Ok(())
}
