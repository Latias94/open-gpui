//! One post-admission loss oracle shared by physical and programmatic native journeys.

use anyhow::{Context as _, Result, ensure};
use open_gpui::{App, AppContext as _, AsyncApp, WindowFrameSubmissionOutcome, px};
use open_gpui_docking::{
    dockspace::{
        geometry::PhysicalRect,
        model::{NativeTearOffSource, NativeWindowPlacement, RootId, SurfaceId},
    },
    native::NativeReport,
};
use open_gpui_windows::{
    NativeWindowTestEventKind, NativeWindowTestMessage,
    native_test_retire_window_without_close_request,
};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

use super::shared_native::run_shared_native_test;
use super::shared_session::{SharedSession, SharedWindow};
use super::{
    assert_native_shutdown_order, native_shutdown_ordinals, raw_hwnd,
    unique_native_identity_for_window, wait_until, window_has_non_empty_frame,
};

pub(super) async fn seed_source_view(cx: &mut AsyncApp, session: &mut SharedSession) -> Result<()> {
    let source = session.source_window()?;
    let view = session
        .source_view
        .clone()
        .context("the shared fixture needs a stateful source View")?;
    cx.update(|app| {
        view.update(app, |view, cx| {
            view.local_edits = 41;
            cx.notify();
        })
    });
    wait_until(cx, "original View state on the source", |cx| {
        session.pump(cx);
        session.check_health()?;
        cx.update(|app| view_presented(app, session, source, 41))
    })
    .await
}

pub(super) fn view_presented(
    app: &mut App,
    session: &SharedSession,
    owner: SharedWindow,
    edits: u64,
) -> Result<bool> {
    let view = session
        .source_view
        .as_ref()
        .context("the shared fixture lost its retained View")?;
    let rendered = view.read(app).last_render == Some((owner.window.window_id(), edits));
    let inspection = session.dock.inspection()?;
    let binding = session
        .dock
        .native_binding_for_window(owner.window.window_id());
    let presented = inspection
        .mounts
        .iter()
        .find(|mount| mount.window_id == owner.window.window_id().as_u64())
        .and_then(|mount| {
            mount
                .regions
                .iter()
                .find(|region| region.surface == owner.surface)
        })
        .is_some_and(|region| {
            region.packet_deferred == Some(false)
                && region.current_presentation
                && region.input_binding_current == Some(true)
                && region.input_native_binding.is_some_and(|placement| {
                    placement.surface() == owner.surface && Some(placement.window()) == binding
                })
        });
    let selector = format!("shared-source-view:{edits}");
    let painted = app.update_window(owner.window, |_, window, _| {
        window
            .committed_debug_bounds_for_test()
            .iter()
            .any(|(candidate, bounds)| {
                candidate == &selector
                    && bounds.size.width > px(0.0)
                    && bounds.size.height > px(0.0)
            })
    })?;
    Ok(rendered && painted && presented && window_has_non_empty_frame(app, owner.window)?)
}

pub(super) async fn wait_admitted_destination(
    cx: &mut AsyncApp,
    session: &mut SharedSession,
) -> Result<SharedWindow> {
    let mut destination = None;
    wait_until(
        cx,
        "exact destination first-live admission and retained View",
        |cx| {
            session.pump(cx);
            session.check_health()?;
            destination = cx.update(|app| admitted_destination(app, session))?;
            Ok(destination.is_some())
        },
    )
    .await
    .with_context(|| format!("destination state: {:?}", session.dock.inspection()))?;
    destination.context("admitted destination disappeared")
}

fn admitted_destination(app: &mut App, session: &SharedSession) -> Result<Option<SharedWindow>> {
    let source = session.source_window()?;
    let Some(surface) = session.dock.with_view(|view| {
        view.item(source.item)
            .map(|item| item.surface())
            .filter(|surface| *surface != source.surface)
    }) else {
        return Ok(None);
    };
    let inspection = session.dock.inspection()?;
    if inspection.pending_native_create {
        return Ok(None);
    }
    for window in app.windows() {
        let Some(owner) = session.dock.native_binding_for_window(window.window_id()) else {
            continue;
        };
        let Some(placement) = inspection
            .mounts
            .iter()
            .find(|mount| mount.window_id == window.window_id().as_u64())
            .and_then(|mount| {
                mount
                    .regions
                    .iter()
                    .find(|region| region.surface == surface)
            })
            .filter(|region| {
                region.input_binding_current == Some(true) && region.current_presentation
            })
            .and_then(|region| region.input_native_binding)
            .filter(|binding| binding.window() == owner)
        else {
            continue;
        };
        let admitted = session.reports.iter().any(|report| {
            matches!(
                report, NativeReport::Admitted(binding)
                    if *binding == placement
            )
        });
        let submitted = session.reports.iter().any(|report| {
            matches!(
                report, NativeReport::Submission { binding, snapshot }
                    if *binding == owner && snapshot.window_id() == window.window_id()
                        && snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted
            )
        });
        if !admitted || !submitted {
            continue;
        }
        let candidate = SharedWindow {
            surface,
            item: source.item,
            window,
            hwnd: raw_hwnd(app, window)?,
            identity: Some(unique_native_identity_for_window(
                &session.observation.events(),
                window.window_id(),
                "admitted destination",
            )?),
        };
        if view_presented(app, session, candidate, 41)? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn recovered_contained_root(
    session: &SharedSession,
    source: SharedWindow,
    lost_surface: SurfaceId,
    root: RootId,
) -> bool {
    session.dock.with_view(|view| {
        view.surface(lost_surface).is_none()
            && view
                .item(source.item)
                .is_some_and(|item| item.surface() == source.surface && item.root() == root)
            && view.surface(source.surface).is_some_and(|surface| {
                surface.contained().any(|contained| {
                    contained
                        .root()
                        .is_some_and(|candidate| candidate.id() == root)
                })
            })
    })
}

#[derive(Clone, Copy)]
enum Recovery {
    Contained,
    Replacement(SharedWindow),
}

pub(super) async fn lose_and_recover_destination(
    cx: &mut AsyncApp,
    session: &mut SharedSession,
    destination: SharedWindow,
) -> Result<()> {
    let source = session.source_window()?;
    let target = session.target_window()?;
    let view = session
        .source_view
        .clone()
        .context("recovery lost the original View")?;
    let identity = view.entity_id();
    let root = session
        .dock
        .with_view(|view| view.item(source.item).map(|item| item.root()))
        .context("admitted View has no root")?;
    let source_main = session
        .dock
        .with_view(|view| {
            view.surface(source.surface)
                .and_then(|surface| surface.main_root())
                .map(|root| root.id())
        })
        .context("recovery source lost its surviving main root")?;
    let binding = session
        .dock
        .native_binding_for_window(destination.window.window_id())
        .context("admitted destination has no native binding")?;
    let shutdown = cx.update(|_| {
        native_test_retire_window_without_close_request(
            destination.window,
            destination.hwnd.0 as isize,
        )
    })?;
    wait_until(cx, "unexpected destination terminal", |cx| {
        session.pump(cx);
        session.check_health()?;
        // A replacement may reuse the raw HWND value. Retirement belongs to
        // the original WindowId, binding and native-generation terminal.
        Ok(!cx.update(|app| app.windows().contains(&destination.window))
            && session.reports.iter().any(
                |report| matches!(report, NativeReport::NativeTerminal(owner) if *owner == binding),
            )
            && session
                .dock
                .native_binding_for_window(destination.window.window_id())
                .is_none())
    })
    .await?;
    let events = session.observation.events();
    let native_identity = destination
        .identity
        .context("loss lost its original native identity")?;
    ensure!(
        !events.iter().any(|event| event.window() == native_identity
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::Close,
                    ..
                }
            )),
        "unexpected loss was replaced by an ordinary WM_CLOSE"
    );
    ensure!(
        !session.reports.iter().any(|report| matches!(
            report, NativeReport::CloseRequested(close) if close.binding().window() == binding
        )),
        "unexpected loss invoked semantic close policy"
    );
    let lifecycle = native_shutdown_ordinals(&events, native_identity)?;
    assert_native_shutdown_order("unexpected destination loss", &lifecycle)?;
    let terminal = shutdown.snapshot();
    ensure!(
        terminal.quiesced()
            && terminal.native_terminal()
            && !terminal.protocol_violation()
            && terminal.generation() == lifecycle.ticket_generation,
        "unexpected loss did not retain its original shutdown authority: {terminal:?}"
    );
    ensure!(
        unsafe { IsWindow(Some(source.hwnd)).as_bool() && IsWindow(Some(target.hwnd)).as_bool() }
            && session
                .dock
                .inspection()?
                .native
                .is_some_and(|native| !native.admission_frozen),
        "child loss drained its surviving workspace"
    );
    // Core may recover into a contained root or admit a native replacement while
    // the recovery scene is unavailable. Prove either automatic outcome before a command.
    let mut recovery = None;
    wait_until(
        cx,
        "core-owned recovery of the original root and View",
        |cx| {
            session.pump(cx);
            session.check_health()?;
            if recovered_contained_root(session, source, destination.surface, root)
                && cx.update(|app| view_presented(app, session, source, 41))?
            {
                recovery = Some(Recovery::Contained);
            } else if let Some(replacement) = cx.update(|app| admitted_destination(app, session))? {
                let replacement_binding = session
                    .dock
                    .native_binding_for_window(replacement.window.window_id());
                ensure!(
                    replacement.window != destination.window
                        && replacement_binding != Some(binding)
                        && replacement.surface == destination.surface
                        && session.dock.with_view(|view| view
                            .item(source.item)
                            .is_some_and(|item| item.root() == root)),
                    "recovery did not retain the original root under a fresh native lifetime"
                );
                recovery = Some(Recovery::Replacement(replacement));
            }
            Ok(recovery.is_some())
        },
    )
    .await
    .with_context(|| format!("automatic recovery state: {:?}", session.dock.inspection()))?;
    let recovery = recovery.context("core recovery disappeared")?;
    let recovered_owner = match recovery {
        Recovery::Contained => source,
        Recovery::Replacement(replacement) => replacement,
    };
    cx.update(|app| {
        view.update(app, |view, cx| {
            view.local_edits += 1;
            cx.notify();
        })
    });
    wait_until(
        cx,
        "same View state presented after automatic recovery",
        |cx| {
            session.pump(cx);
            session.check_health()?;
            cx.update(|app| view_presented(app, session, recovered_owner, 42))
        },
    )
    .await?;
    if let Recovery::Replacement(replacement) = recovery {
        let replacement_binding = session
            .dock
            .native_binding_for_window(replacement.window.window_id())
            .context("replacement lost its native binding")?;
        cx.update(|app| session.dock.submit_native_dock_back_for_test(root, app))?;
        wait_until(cx, "shared dock-back and replacement retirement", |cx| {
            session.pump(cx);
            session.check_health()?;
            Ok(recovered_contained_root(session, source, destination.surface, root)
                && cx.update(|app| view_presented(app, session, source, 42))?
                && !cx.update(|app| app.windows().contains(&replacement.window))
                && session.dock.native_binding_for_window(replacement.window.window_id()).is_none()
                && session.reports.iter().any(|report| matches!(report, NativeReport::NativeTerminal(owner) if *owner == replacement_binding)))
        }).await.with_context(|| format!("dock-back state: {:?}", session.dock.inspection()))?;
        let replacement_lifecycle = native_shutdown_ordinals(
            &session.observation.events(),
            replacement
                .identity
                .context("replacement lost its native identity")?,
        )?;
        assert_native_shutdown_order("dock-back replacement", &replacement_lifecycle)?;
        session
            .milestones
            .push("shared.committed-loss-replacement-retired".to_owned());
    } else {
        session
            .milestones
            .push("shared.committed-loss-contained-recovery".to_owned());
    }
    cx.update(|app| session.dock.submit_native_dock_back_for_test(root, app))?;
    wait_until(
        cx,
        "recovered View docked into the original source main root",
        |cx| {
            session.pump(cx);
            session.check_health()?;
            let docked = session.dock.with_view(|view| {
                view.surface(destination.surface).is_none()
                    && view.root(root).is_none()
                    && view.item(source.item).is_some_and(|item| {
                        item.surface() == source.surface
                            && item.root() == source_main
                            && item.is_selected()
                    })
            });
            Ok(docked && cx.update(|app| view_presented(app, session, source, 42))?)
        },
    )
    .await?;
    ensure!(
        view.entity_id() == identity && cx.update(|app| view.read(app).local_edits) == 42,
        "recovery replaced or reset the original View"
    );
    ensure!(
        cx.update(|app| {
            let windows = app.windows();
            windows.len() == session.preexisting_window_count + 2
                && windows.contains(&source.window)
                && windows.contains(&target.window)
        }) && session.dock.inspection()?.open_pane_count == 3,
        "dock-back changed the surviving window or pane roster"
    );
    session
        .milestones
        .push("shared.committed-loss-dock-back".to_owned());
    Ok(())
}

#[test]
fn real_windows_committed_destination_loss_recovers_the_same_view_without_system_input() {
    run_shared_native_test(None, |cx, session| {
        Box::pin(async move {
            seed_source_view(cx, session).await?;
            cx.update(|app| {
                session.dock.update(
                    |frame| {
                        frame.move_to_new_window_current(
                            NativeTearOffSource::Item(session.source_window()?.item),
                            NativeWindowPlacement::new(PhysicalRect::new(
                                150.0, 180.0, 720.0, 540.0,
                            )?),
                        )?;
                        Ok(())
                    },
                    app,
                )
            })?;
            let destination = wait_admitted_destination(cx, session).await?;
            let source = session.source_window()?;
            ensure!(
                cx.update(|_| native_test_retire_window_without_close_request(
                    source.window,
                    destination.hwnd.0 as isize,
                ))
                .is_err()
                    && unsafe { IsWindow(Some(destination.hwnd)).as_bool() },
                "a mismatched native loss target retired another window"
            );
            lose_and_recover_destination(cx, session, destination).await
        })
    });
}
