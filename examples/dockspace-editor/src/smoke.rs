//! A bounded native consumer of the production adapter, without a second renderer.

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail, ensure};
use open_gpui::{AnyWindowHandle, App, Entity, WindowFrameSubmissionOutcome, WindowOptions};
use open_gpui_docking::{
    dockspace::{
        geometry::PhysicalRect,
        model::NativeWindowPlacement,
        runtime::{NativeSurfaceBinding, NativeWindowBinding},
    },
    native::{NativeCloseDecision, NativeReport},
};

use super::{
    DockAnchor, DockPlacement, EDITOR, EditorView, FIRST, INSPECTOR, SECOND, ViewFrame, Workspace,
};

#[derive(Debug, Clone, Copy)]
enum Phase {
    RootSubmitted,
    ChildAdmitted,
    Edited,
    Moved,
    EditedAfterMove,
    GroupRetired,
    SiblingEdited,
    SiblingRetired,
}

struct Smoke {
    workspace: Workspace,
    root: AnyWindowHandle,
    child: Option<AnyWindowHandle>,
    sibling: Workspace,
    sibling_window: AnyWindowHandle,
    phase: Phase,
    submitted: BTreeSet<NativeWindowBinding>,
    terminals: BTreeSet<NativeWindowBinding>,
    subject_bindings: BTreeSet<NativeWindowBinding>,
    admitted: Option<NativeSurfaceBinding>,
    sibling_terminal: bool,
}

pub(super) fn run(workspace: Workspace, root: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let sibling = super::workspace(cx)?;
    let sibling_window =
        sibling
            .dock
            .open_native_root(EDITOR, true, WindowOptions::default(), cx)?;
    let mut smoke = Smoke {
        workspace,
        root,
        child: None,
        sibling,
        sibling_window,
        phase: Phase::RootSubmitted,
        submitted: BTreeSet::new(),
        terminals: BTreeSet::new(),
        subject_bindings: BTreeSet::new(),
        admitted: None,
        sibling_terminal: false,
    };
    let clock = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match cx.update(|cx| smoke.step(cx)) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => fail(format!("{:?}: {error:#}", smoke.phase)),
            }
            if Instant::now() >= deadline {
                fail(format!(
                    "timed out after 30s in {:?}; admitted={:?}; update pending={}; submitted={:?}; native terminals={:?}",
                    smoke.phase, smoke.admitted, smoke.workspace.dock.is_update_pending(), smoke.submitted, smoke.terminals
                ));
            }
            clock.timer(Duration::from_millis(16)).await;
        }
    })
    .detach();
    Ok(())
}

fn fail(message: String) -> ! {
    eprintln!("DOCKSPACE NATIVE SMOKE FAILED: {message}");
    std::process::exit(1)
}

impl Smoke {
    fn step(&mut self, cx: &mut App) -> Result<bool> {
        self.workspace.dock.pump_native(cx);
        self.sibling.dock.pump_native(cx);
        ensure!(
            self.workspace.dock.error().is_none(),
            "subject adapter: {:?}",
            self.workspace.dock.error()
        );
        ensure!(
            self.sibling.dock.error().is_none(),
            "sibling adapter: {:?}",
            self.sibling.dock.error()
        );
        self.reports()?;
        match self.phase {
            Phase::RootSubmitted => {
                if presented(&self.workspace.views[0], self.root, 0, 0, cx)?.is_none()
                    || presented(&self.sibling.views[0], self.sibling_window, 0, 0, cx)?.is_none()
                    || self.workspace.dock.is_update_pending()
                {
                    return Ok(false);
                }
                let binding = self
                    .workspace
                    .dock
                    .native_binding_for_window(self.root.window_id())
                    .context("root has its original native binding")?;
                if !self.submitted.contains(&binding) {
                    return Ok(false);
                }
                // This is a requested outer frame, not fabricated observed geometry.
                // The bridge must use the real owning backend's target-DPI conversion.
                self.workspace.dock.update(
                    |frame| {
                        frame.create_native_surface_current(
                            INSPECTOR,
                            EDITOR,
                            NativeWindowPlacement::new(PhysicalRect::new(
                                180.0, 140.0, 760.0, 560.0,
                            )?),
                        )?;
                        Ok(())
                    },
                    cx,
                )?;
                self.phase = Phase::ChildAdmitted;
            }
            Phase::ChildAdmitted => {
                let Some(admitted) = self.admitted else {
                    return Ok(false);
                };
                let child = cx
                    .windows()
                    .into_iter()
                    .find(|window| {
                        self.workspace
                            .dock
                            .native_binding_for_window(window.window_id())
                            == Some(admitted.window())
                    })
                    .context("admitted child owns an actual GPUI Window")?;
                if !self.submitted.contains(&admitted.window())
                    || presented(&self.workspace.views[1], child, 0, 0, cx)?.is_none()
                {
                    return Ok(false);
                }
                ensure!(
                    child != self.root && child != self.sibling_window,
                    "native creation reused a physical window"
                );
                self.child = Some(child);
                self.subject_bindings.insert(admitted.window());
                self.subject_bindings.insert(
                    self.workspace
                        .dock
                        .native_binding_for_window(self.root.window_id())
                        .unwrap(),
                );
                ensure!(
                    self.subject_bindings.len() == 2,
                    "two distinct physical lifetimes are required"
                );
                self.workspace.views[0].update(cx, |view, cx| view.edit(cx));
                self.phase = Phase::Edited;
            }
            Phase::Edited => {
                let child = self.child.context("native child")?;
                if presented(&self.workspace.views[0], self.root, 1, 1, cx)?.is_none()
                    || presented(&self.workspace.views[1], child, 0, 1, cx)?.is_none()
                    || self.workspace.dock.is_update_pending()
                {
                    return Ok(false);
                }
                self.workspace.dock.update(
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
                ensure!(
                    self.workspace
                        .dock
                        .with_view(|view| view.item(FIRST).map(|item| item.surface()))
                        == Some(INSPECTOR),
                    "the committed move did not reach the destination surface"
                );
                self.phase = Phase::Moved;
            }
            Phase::Moved => {
                let child = self.child.context("native child")?;
                if presented(&self.workspace.views[0], child, 1, 1, cx)?.is_none()
                    || view_is_still_presented(&self.workspace.views[0], self.root, cx)?
                {
                    return Ok(false);
                }
                ensure!(
                    self.workspace.views[1].read(cx).draft == 0,
                    "another View's local draft changed"
                );
                self.workspace.views[0].update(cx, |view, cx| view.edit(cx));
                self.phase = Phase::EditedAfterMove;
            }
            Phase::EditedAfterMove => {
                if presented(&self.workspace.views[0], self.child.unwrap(), 2, 2, cx)?.is_none() {
                    return Ok(false);
                }
                ensure!(
                    self.workspace.document.borrow().edits == 2,
                    "shared document edits were lost"
                );
                self.workspace
                    .dock
                    .set_native_close_policy(NativeCloseDecision::Defer)?;
                self.workspace.dock.begin_native_shutdown(cx)?;
                self.phase = Phase::GroupRetired;
            }
            Phase::GroupRetired => {
                if !self.workspace.dock.native_is_quiescent()
                    || !self.subject_bindings.is_subset(&self.terminals)
                {
                    return Ok(false);
                }
                ensure!(
                    !cx.windows().contains(&self.root)
                        && !cx.windows().contains(&self.child.unwrap()),
                    "retired windows remain in GPUI's logical registry"
                );
                ensure!(
                    cx.windows().contains(&self.sibling_window)
                        && !self.sibling.dock.native_is_quiescent(),
                    "the isolated sibling workspace did not survive group shutdown"
                );
                self.sibling.views[0].update(cx, |view, cx| view.edit(cx));
                self.phase = Phase::SiblingEdited;
            }
            Phase::SiblingEdited => {
                if presented(&self.sibling.views[0], self.sibling_window, 1, 1, cx)?.is_none() {
                    return Ok(false);
                }
                ensure!(
                    self.workspace.document.borrow().edits == 2,
                    "sibling edited another workspace's document"
                );
                self.sibling.dock.begin_native_shutdown(cx)?;
                self.phase = Phase::SiblingRetired;
            }
            Phase::SiblingRetired => {
                if !self.sibling_terminal || !self.sibling.dock.native_is_quiescent() {
                    return Ok(false);
                }
                ensure!(
                    cx.windows().is_empty(),
                    "native smoke left an unretired window"
                );
                println!(
                    "DOCKSPACE NATIVE SMOKE OK: created child; 2 real Submitted windows; retained entity and local/shared edits; dependent/anchor native terminal and binding quiescence; isolated sibling survived and retired"
                );
                cx.quit();
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn reports(&mut self) -> Result<()> {
        for report in self.workspace.dock.take_native_reports() {
            match report {
                NativeReport::Submission { binding, snapshot }
                    if snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted
                        && snapshot.accepted_frame_generation().is_some()
                        && snapshot.accepted_frame_generation()
                            == snapshot.submitted_frame_generation() =>
                {
                    self.submitted.insert(binding);
                }
                NativeReport::Admitted(binding) if binding.surface() == INSPECTOR => {
                    self.admitted = Some(binding)
                }
                NativeReport::NativeTerminal(binding) => {
                    self.terminals.insert(binding);
                }
                NativeReport::OpenFailed { stage, message, .. } => {
                    bail!("native create failed at {stage:?}: {message}")
                }
                NativeReport::Unsupported { operation } => {
                    bail!("native operation unsupported: {operation}")
                }
                _ => {}
            }
        }
        for report in self.sibling.dock.take_native_reports() {
            match report {
                NativeReport::NativeTerminal(_) => self.sibling_terminal = true,
                NativeReport::OpenFailed { message, .. } => {
                    bail!("sibling native failure: {message}")
                }
                NativeReport::Unsupported { operation } => {
                    bail!("sibling operation unsupported: {operation}")
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn presented(
    view: &Entity<EditorView>,
    window: AnyWindowHandle,
    draft: usize,
    edits: usize,
    cx: &mut App,
) -> Result<Option<ViewFrame>> {
    let frame = view
        .read(cx)
        .frames
        .borrow()
        .get(&window.window_id())
        .copied();
    let Some(frame) = frame else { return Ok(None) };
    ensure!(
        frame.entity == view.entity_id(),
        "the adapter replaced the original View entity"
    );
    if frame.draft != draft || frame.document_edits != edits {
        return Ok(None);
    }
    let facts = window.update(cx, |_, window, _| window.presentation_facts())?;
    Ok((facts.native_created
        && facts.native_visible
        && facts.present_submitted_generation == Some(frame.generation)
        && facts.non_empty_presented_generation == Some(frame.generation))
    .then_some(frame))
}

fn view_is_still_presented(
    view: &Entity<EditorView>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<bool> {
    let frame = view
        .read(cx)
        .frames
        .borrow()
        .get(&window.window_id())
        .copied();
    let facts = window.update(cx, |_, window, _| window.presentation_facts())?;
    Ok(frame.is_some_and(|frame| facts.present_submitted_generation == Some(frame.generation)))
}
