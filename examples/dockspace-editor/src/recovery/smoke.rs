//! Programmatic fault injection with actual native windows and renderer terminals.

use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, ensure};
use open_gpui::{App, Entity};
use open_gpui_docking::{
    DockSurface,
    dockspace::runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
        NativePointerEvent, NativePointerHover, NativePointerId, NativePointerInput,
        NativePointerOwner,
    },
};

use super::*;
use crate::{DockAnchor, DockPlacement, EditorView, FIRST, INSPECTOR, SECOND, smoke::presented};

#[derive(Clone, Copy, Debug)]
enum Step {
    Initial,
    EditedAndMoved,
    FaultRecorded,
    Rebuilt,
    EditedAfterRecovery,
    Closed,
}

struct Smoke {
    recovery: Entity<Recovery>,
    original: DockSurface,
    original_roots: BTreeMap<SurfaceId, AnyWindowHandle>,
    original_views: [Entity<EditorView>; 2],
    original_bindings: Vec<open_gpui_docking::dockspace::runtime::NativeWindowBinding>,
    step: Step,
}

pub(crate) fn run(recovery: Entity<Recovery>, cx: &mut App) -> Result<()> {
    let mut smoke = {
        let owner = recovery.read(cx);
        ensure!(
            owner.phase == Phase::Running && owner.roots.len() == 2,
            "the initial workspace did not open completely: {}",
            owner.notice
        );
        Smoke {
            recovery: recovery.clone(),
            original: owner.workspace.dock.clone(),
            original_roots: owner.roots.clone(),
            original_views: owner.workspace.views.clone(),
            original_bindings: owner
                .roots
                .values()
                .map(|window| {
                    owner
                        .workspace
                        .dock
                        .native_binding_for_window(window.window_id())
                        .context("original root binding")
                })
                .collect::<Result<_>>()?,
            step: Step::Initial,
        }
    };
    let clock = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match cx.update(|cx| smoke.step(cx)) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => fail(format!("{:?}: {error:#}", smoke.step)),
            }
            if Instant::now() >= deadline {
                let diagnostic = cx.update(|cx| smoke.diagnostic(cx));
                fail(format!("timed out in {:?}: {diagnostic}", smoke.step));
            }
            clock.timer(Duration::from_millis(16)).await;
        }
    })
    .detach();
    Ok(())
}

fn fail(message: String) -> ! {
    eprintln!("DOCKSPACE RECOVERY SMOKE FAILED: {message}");
    std::process::exit(1)
}

impl Smoke {
    fn diagnostic(&self, cx: &mut App) -> String {
        let (roots, views, failure) = {
            let owner = self.recovery.read(cx);
            (
                owner.roots.clone(),
                owner.workspace.views.clone(),
                owner.workspace.dock.failure(),
            )
        };
        let frames = views.map(|view| view.read(cx).frames.borrow().clone());
        let facts = roots
            .into_iter()
            .map(|(surface, window)| {
                (
                    surface,
                    window.update(cx, |_, window, _| window.presentation_facts()),
                )
            })
            .collect::<Vec<_>>();
        format!(
            "failure={failure:?}; frames={frames:?}; facts={facts:?}; windows={}",
            cx.debug_state()["windows"]
        )
    }

    fn step(&mut self, cx: &mut App) -> Result<bool> {
        let (dock, views, roots, phase) = {
            let owner = self.recovery.read(cx);
            (
                owner.workspace.dock.clone(),
                owner.workspace.views.clone(),
                owner.roots.clone(),
                owner.phase,
            )
        };
        dock.pump_native(cx);
        match self.step {
            Step::Initial => {
                if dock.is_update_pending() {
                    return Ok(false);
                }
                for (view, surface) in views.iter().zip([EDITOR, INSPECTOR]) {
                    if presented(view, roots[&surface], 0, 0, cx)?.is_none() {
                        return Ok(false);
                    }
                }
                views[0].update(cx, |view, cx| view.edit(cx));
                dock.update(
                    |frame| {
                        frame.dock_item_current(
                            FIRST,
                            DockPlacement::Center(DockAnchor::Item(SECOND)),
                        )?;
                        frame.select_item_current(FIRST)?;
                        Ok(())
                    },
                    cx,
                )?;
                self.step = Step::EditedAndMoved;
            }
            Step::EditedAndMoved => {
                if presented(&views[0], roots[&INSPECTOR], 1, 1, cx)?.is_none()
                    || dock.is_update_pending()
                {
                    return Ok(false);
                }
                let binding = dock
                    .native_binding_for_window(roots[&EDITOR].window_id())
                    .context("original native root binding")?;
                let transport = dock.native_transport().context("native ingress")?;
                // Intentionally malformed protocol input. These are not physical
                // mouse events and grant no OS capture or global-position facts.
                for _ in 0..2 {
                    transport.pointer(
                        NativePointerInput::new(
                            NativePointerId::new(71),
                            NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
                            NativeDesktopPointerLocation::new(
                                NativeDesktopPosition::Unknown,
                                NativePointerHover::Unknown,
                                None,
                            ),
                            NativePointerOwner::Native(binding),
                            NativePointerOwner::None,
                        ),
                        cx,
                    );
                }
                self.step = Step::FaultRecorded;
            }
            Step::FaultRecorded => {
                let Some(failure) = dock.failure() else {
                    return Ok(false);
                };
                ensure!(
                    failure.runtime_kind == Some(DockspaceRuntimeErrorKind::HostProtocol),
                    "unexpected diagnostic: {failure:?}"
                );
                ensure!(
                    phase == Phase::Running,
                    "diagnostics started teardown automatically"
                );
                self.recovery.update(cx, |owner, cx| owner.rebuild(cx));
                self.step = Step::Rebuilt;
            }
            Step::Rebuilt => {
                if phase != Phase::Running
                    || !matches!(
                        self.original.native_termination(),
                        Some(DockNativeTermination::Failed(_))
                    )
                {
                    return Ok(false);
                }
                ensure!(
                    !self.original.native_is_quiescent(),
                    "failed termination claimed semantic drain"
                );
                ensure!(
                    self.original.inspection()?.pending_native_input,
                    "the original failed prefix was discarded"
                );
                ensure!(
                    dock.failure().is_none() && !dock.inspection()?.pending_native_input,
                    "old failure or input entered the successor"
                );
                ensure!(
                    views == self.original_views,
                    "recovery replaced an original View entity"
                );
                for (surface, window) in &roots {
                    ensure!(
                        *window != self.original_roots[surface],
                        "reused an old GPUI window lifetime"
                    );
                    ensure!(
                        !cx.windows().contains(&self.original_roots[surface]),
                        "original root remains live"
                    );
                    let binding = dock
                        .native_binding_for_window(window.window_id())
                        .context("successor native binding")?;
                    ensure!(
                        !self.original_bindings.contains(&binding),
                        "reused native provider authority"
                    );
                }
                if presented(&views[0], roots[&INSPECTOR], 1, 1, cx)?.is_none() {
                    return Ok(false);
                }
                views[0].update(cx, |view, cx| view.edit(cx));
                self.step = Step::EditedAfterRecovery;
            }
            Step::EditedAfterRecovery => {
                if presented(&views[0], roots[&INSPECTOR], 2, 2, cx)?.is_none() {
                    return Ok(false);
                }
                ensure!(
                    dock.failure().is_none(),
                    "the successor failed after recovery: {:?}",
                    dock.failure()
                );
                ensure!(
                    self.recovery.read(cx).workspace.document.borrow().edits == 2,
                    "document edits were lost"
                );
                self.recovery.update(cx, |owner, cx| owner.close(cx));
                self.step = Step::Closed;
            }
            Step::Closed => {
                if !cx.windows().is_empty() || dock.native_termination().is_none() {
                    return Ok(false);
                }
                ensure!(
                    dock.native_termination() == Some(DockNativeTermination::Drained),
                    "the healthy successor did not drain normally"
                );
                println!(
                    "DOCKSPACE RECOVERY SMOKE OK: real Submitted roots; explicit protocol fault; failed physical termination; retained original Views and committed move; fresh native session; edits after recovery; ordered final shutdown"
                );
                cx.quit();
                return Ok(true);
            }
        }
        Ok(false)
    }
}
