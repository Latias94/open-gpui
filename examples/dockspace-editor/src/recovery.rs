//! The application owns reconstruction; a DockSurface owns exactly one generation.

use anyhow::Result;
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement, Render,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowOptions, div, point, prelude::*, px,
    rgb, size,
};
use open_gpui_docking::{
    DockNativeTermination,
    dockspace::{model::DockspaceLayout, runtime::DockspaceRuntimeErrorKind},
};

use super::{BTreeMap, EDITOR, SurfaceId, Workspace, create_dock};

mod smoke;
pub(super) use smoke::run as run_smoke;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AfterRetirement {
    Rebuild,
    Retry,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Running,
    Retiring(AfterRetirement),
    Ready,
    Closing,
}

pub(super) struct Recovery {
    pub(super) workspace: Workspace,
    pub(super) roots: BTreeMap<SurfaceId, AnyWindowHandle>,
    control: Option<AnyWindowHandle>,
    checkpoint: DockspaceLayout,
    phase: Phase,
    last_termination: Option<DockNativeTermination>,
    notice: String,
    _quit: Subscription,
}

pub(super) fn open(workspace: Workspace, cx: &mut App) -> Result<Entity<Recovery>> {
    let checkpoint = workspace.dock.with_view(|view| view.to_layout());
    let recovery = cx.new(|cx| Recovery {
        workspace,
        roots: BTreeMap::new(),
        control: None,
        checkpoint,
        phase: Phase::Running,
        last_termination: None,
        notice: "Docking workspace ready.".into(),
        _quit: cx.on_app_quit(|recovery: &mut Recovery, _| {
            // An already queued termination callback must not reopen windows
            // while App is executing its process-wide shutdown transaction.
            recovery.phase = Phase::Closing;
            std::future::ready(())
        }),
    });
    let weak = recovery.downgrade();
    let control = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(460.0), px(320.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Dockspace Editor / Recovery".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let _ = weak.update(cx, |recovery, cx| recovery.close(cx));
                false
            });
            recovery.clone()
        },
    )?;
    recovery.update(cx, |recovery, cx| {
        recovery.control = Some(control.into());
        recovery.open_roots(cx);
    });
    Ok(recovery)
}

impl Recovery {
    fn open_roots(&mut self, cx: &mut Context<Self>) {
        let surfaces = self.workspace.dock.with_view(|view| {
            view.surfaces()
                .map(|surface| surface.id())
                .collect::<Vec<_>>()
        });
        let anchor = surfaces
            .iter()
            .copied()
            .find(|surface| *surface == EDITOR)
            .or_else(|| surfaces.first().copied());
        let display = cx
            .primary_display()
            .map(|display| (display.id(), display.visible_bounds()));
        let count = surfaces.len() as f32;
        for (index, surface) in surfaces.into_iter().enumerate() {
            // Give each root its own visible area on startup and reconstruction.
            let window_bounds = display.map(|(_, area)| {
                let column = area.size.width / count;
                WindowBounds::Windowed(Bounds::new(
                    area.origin + point(column * (index as f32 + 0.04), area.size.height * 0.1),
                    size(column * 0.92, area.size.height * 0.8),
                ))
            });
            match self.workspace.dock.open_native_root(
                surface,
                Some(surface) == anchor,
                WindowOptions {
                    window_bounds,
                    display_id: display.map(|(display_id, _)| display_id),
                    ..WindowOptions::default()
                },
                cx,
            ) {
                Ok(window) => {
                    self.roots.insert(surface, window);
                }
                Err(error) => {
                    self.notice = format!("Opening docking windows failed: {error:#}");
                    // Retain the new host and its original rollback tickets even
                    // when only some roots opened. Retry is an explicit action.
                    self.retire(AfterRetirement::Retry, cx);
                    break;
                }
            }
        }
        cx.notify();
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        match self.phase {
            Phase::Running => {
                // Capture before drain: closing a Child can recover its content
                // into the anchor and change the retiring generation's layout.
                self.checkpoint = self.workspace.dock.with_view(|view| view.to_layout());
                self.notice = "Waiting for the original docking windows to terminate.".into();
                self.retire(AfterRetirement::Rebuild, cx);
            }
            Phase::Ready => self.restore(cx),
            Phase::Retiring(AfterRetirement::Close) | Phase::Closing => {}
            Phase::Retiring(_) => self.drive_retirement(cx),
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Closing {
            return;
        }
        if self.phase == Phase::Ready {
            self.close_control(cx);
        } else {
            self.notice = "Closing the docking workspace.".into();
            self.retire(AfterRetirement::Close, cx);
        }
    }

    fn retire(&mut self, after: AfterRetirement, cx: &mut Context<Self>) {
        if !matches!(self.phase, Phase::Retiring(_)) {
            let weak = cx.weak_entity();
            if let Err(error) = self.workspace.dock.when_native_terminated(
                move |termination, cx| {
                    // A native pump can complete synchronously inside rebuild.
                    // Reenter the application owner only after its borrow ends.
                    cx.defer(move |cx| {
                        let _ = weak.update(cx, |recovery, cx| {
                            recovery.terminated(termination, cx);
                        });
                    });
                },
                cx,
            ) {
                self.notice = format!("Cannot observe docking termination: {error:#}");
                cx.notify();
                return;
            }
        }
        self.phase = Phase::Retiring(after);
        self.drive_retirement(cx);
    }

    fn drive_retirement(&mut self, cx: &mut Context<Self>) {
        let failure = self.workspace.dock.failure();
        let failed = failure.is_some_and(|failure| {
            matches!(
                failure.runtime_kind,
                Some(
                    DockspaceRuntimeErrorKind::HostProtocol
                        | DockspaceRuntimeErrorKind::Internal
                        | DockspaceRuntimeErrorKind::ForeignUpdate
                )
            )
        });
        // This is the demo's explicit rebuild/close policy. Merely observing a
        // failure never starts termination or attempts to repair the old FIFO.
        let result = if failed {
            self.workspace.dock.begin_failed_native_shutdown(cx)
        } else {
            self.workspace.dock.begin_native_shutdown(cx)
        };
        if let Err(error) = result {
            self.notice = format!("Docking termination needs attention: {error:#}");
        }
        cx.notify();
    }

    fn terminated(&mut self, termination: DockNativeTermination, cx: &mut Context<Self>) {
        let Phase::Retiring(after) = self.phase else {
            return;
        };
        self.last_termination = Some(termination);
        self.roots.clear();
        self.phase = Phase::Ready;
        match after {
            AfterRetirement::Rebuild => self.restore(cx),
            AfterRetirement::Retry => cx.notify(),
            AfterRetirement::Close => self.close_control(cx),
        }
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        match create_dock(self.checkpoint.clone(), &self.workspace.views, cx) {
            Ok(dock) => {
                // Only application data and committed topology cross generations.
                // Replacing the field now releases the complete terminated host.
                self.workspace.dock = dock;
                for view in &self.workspace.views {
                    view.read(cx).frames.borrow_mut().clear();
                }
                self.phase = Phase::Running;
                self.notice = match self.last_termination {
                    Some(DockNativeTermination::Failed(_)) => {
                        "Recovered after a host failure. Your Views and edits were retained."
                    }
                    _ => "Rebuilt with the original Views and document edits.",
                }
                .into();
                self.open_roots(cx);
            }
            Err(error) => {
                self.notice = format!("Workspace reconstruction failed: {error:#}");
                cx.notify();
            }
        }
    }

    fn close_control(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Closing;
        if let Some(control) = self.control {
            cx.defer(move |cx| {
                let _ = control.update(cx, |_, window, cx| window.remove_window(cx));
            });
        }
    }
}

impl Render for Recovery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = match self.phase {
            Phase::Running if self.workspace.dock.native_termination().is_some() => {
                "Docking windows closed"
            }
            Phase::Running => "Running",
            Phase::Retiring(_) => "Retiring original windows",
            Phase::Ready => "Ready to retry reconstruction",
            Phase::Closing => "Closing",
        };
        let failure = self.workspace.dock.failure();
        div()
            .id("workspace-recovery")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(0xfafafa))
            .text_color(rgb(0x29313d))
            .child("Dockspace Editor / Recovery")
            .child(format!(
                "{status} · {} document edits",
                self.workspace.document.borrow().edits
            ))
            .child(
                div()
                    .id("rebuild-workspace")
                    .debug_selector(|| "rebuild-workspace".into())
                    .px_3()
                    .py_2()
                    .bg(rgb(0xe2edfc))
                    .on_click(cx.listener(|recovery, _, _, cx| recovery.rebuild(cx)))
                    .child(match self.phase {
                        Phase::Running => "Rebuild docking windows",
                        Phase::Retiring(_) => "Retry window shutdown",
                        Phase::Ready => "Retry reconstruction",
                        Phase::Closing => "Closing",
                    }),
            )
            .child(
                div()
                    .id("inspect-workspace")
                    .on_click(cx.listener(|_, _, _, cx| cx.notify()))
                    .child("Inspect workspace status"),
            )
            .child(self.notice.clone())
            .when_some(failure, |element, failure| {
                element.child(format!("Host diagnostic: {}", failure.message))
            })
            .child("Closing this control window retires the docking workspace first.")
    }
}
