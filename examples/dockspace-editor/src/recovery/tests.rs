use std::time::Duration;

use open_gpui::{
    Modifiers, PlatformWindowPresentOutcome, QuitMode, RequestFrameOptions, TestAppContext,
    VisualTestContext, WindowMutationDomain,
};
use open_gpui_docking::dockspace::{
    geometry::PhysicalRect,
    model::NativeWindowPlacement,
    runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
        NativePointerEvent, NativePointerHover, NativePointerId, NativePointerInput,
        NativePointerOwner,
    },
};

use super::*;
use crate::{DockAnchor, DockPlacement, FIRST, INSPECTOR, SECOND, workspace};

fn editor(cx: &mut TestAppContext) -> Entity<Recovery> {
    let recovery = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        open(workspace(cx).unwrap(), cx).unwrap()
    });
    settle(cx, &recovery);
    recovery
}

fn settle(cx: &mut TestAppContext, recovery: &Entity<Recovery>) {
    cx.run_until_parked();
    let dock = current(cx, recovery);
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
}

fn current(cx: &mut TestAppContext, recovery: &Entity<Recovery>) -> open_gpui_docking::DockSurface {
    cx.update(|cx| recovery.read(cx).workspace.dock.clone())
}

fn edit_and_move(cx: &mut TestAppContext, recovery: &Entity<Recovery>) {
    cx.update(|cx| {
        let (view, dock) = {
            let owner = recovery.read(cx);
            (
                owner.workspace.views[0].clone(),
                owner.workspace.dock.clone(),
            )
        };
        view.update(cx, |view, cx| view.edit(cx));
        dock.update(
            |frame| {
                frame.dock_item_current(FIRST, DockPlacement::Center(DockAnchor::Item(SECOND)))?;
                frame.select_item_current(FIRST)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    settle(cx, recovery);
}

#[open_gpui::test]
fn rebuild_waits_for_original_terminals_and_preserves_views_and_isolated_workspace(
    cx: &mut TestAppContext,
) {
    let recovery = editor(cx);
    let dock = current(cx, &recovery);
    let detached = SurfaceId::new(3);
    let detached_root = crate::RootId::new(3);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.attach_empty_surface_current(detached, detached_root)?;
                frame.dock_item_current(
                    SECOND,
                    DockPlacement::Center(DockAnchor::Central(detached_root)),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    settle(cx, &recovery);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.create_native_surface_current(
                    detached,
                    EDITOR,
                    NativeWindowPlacement::new(PhysicalRect::new(100.0, 100.0, 600.0, 400.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    settle(cx, &recovery);
    let child = cx.update(|cx| {
        let roots = &recovery.read(cx).roots;
        cx.windows()
            .into_iter()
            .find(|window| {
                !roots.values().any(|root| root == window)
                    && dock
                        .native_callbacks_for_window(window.window_id())
                        .is_some()
            })
            .expect("the requested surface must create a managed Child")
    });
    assert!(cx.flush_window_mutation(child, WindowMutationDomain::Placement));
    assert!(
        VisualTestContext::from_window(child, cx).simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        })
    );
    settle(cx, &recovery);
    assert!(dock.failure().is_none(), "{:?}", dock.failure());
    edit_and_move(cx, &recovery);
    assert_eq!(
        dock.with_view(|view| view.item(FIRST).unwrap().surface()),
        detached
    );
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.request_close_item_current(SECOND)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    settle(cx, &recovery);
    assert!(dock.with_view(|view| view.item(SECOND).is_none()));
    let sibling = cx.update(workspace).unwrap();
    let sibling_window = cx
        .update(|cx| {
            sibling
                .dock
                .open_native_root(EDITOR, true, WindowOptions::default(), cx)
        })
        .unwrap();
    settle(cx, &recovery);
    let old = current(cx, &recovery);
    let (control, mut roots, views) = cx.update(|cx| {
        let owner = recovery.read(cx);
        (
            owner.control.unwrap(),
            owner.roots.clone(),
            owner.workspace.views.clone(),
        )
    });
    roots.insert(detached, child);
    let binding = old.native_binding_for_window(child.window_id()).unwrap();
    let renderer = cx.hold_window_presentation_shutdown(child);
    let terminal = cx.hold_window_native_terminal(child);
    cx.update(|cx| {
        recovery.update(cx, |owner, cx| {
            owner.rebuild(cx);
            owner.rebuild(cx);
        })
    });
    settle(cx, &recovery);
    cx.update(|cx| {
        assert_eq!(
            recovery.read(cx).phase,
            Phase::Retiring(AfterRetirement::Rebuild)
        );
        assert!(cx.windows().contains(&control));
    });
    assert!(old.native_termination().is_none());
    renderer.release();
    cx.background_executor
        .advance_clock(Duration::from_millis(512));
    settle(cx, &recovery);
    assert!(
        old.native_termination().is_none(),
        "renderer shutdown cannot stand in for native terminal"
    );
    cx.update(|cx| {
        assert_eq!(
            recovery.read(cx).phase,
            Phase::Retiring(AfterRetirement::Rebuild)
        )
    });
    assert!(terminal.release());
    settle(cx, &recovery);
    assert_eq!(
        old.native_termination(),
        Some(DockNativeTermination::Drained)
    );
    let fresh = current(cx, &recovery);
    cx.update(|cx| {
        let owner = recovery.read(cx);
        assert_eq!(owner.phase, Phase::Running);
        assert_eq!(owner.last_termination, Some(DockNativeTermination::Drained));
        assert_eq!(owner.workspace.views, views);
        assert_eq!(views[0].read(cx).draft, 1);
        assert_eq!(owner.workspace.document.borrow().edits, 1);
        for (surface, window) in &owner.roots {
            assert_ne!(*window, roots[surface]);
        }
        assert_ne!(
            fresh
                .native_binding_for_window(owner.roots[&detached].window_id())
                .unwrap(),
            binding
        );
        assert!(cx.windows().contains(&sibling_window));
    });
    assert!(!cx.did_quit());
    assert_eq!(
        fresh.with_view(|view| view.item(FIRST).unwrap().surface()),
        detached
    );
    assert!(
        fresh.with_view(|view| view.item(SECOND).is_none()),
        "rebuild must not reopen a closed View"
    );
    assert!(fresh.error().is_none(), "{:?}", fresh.error());
    cx.update(|cx| {
        views[0].update(cx, |view, cx| view.edit(cx));
        sibling.views[0].update(cx, |view, cx| view.edit(cx));
        assert_eq!(views[0].read(cx).draft, 2);
        assert_eq!(sibling.document.borrow().edits, 1);
        recovery.update(cx, |owner, cx| owner.close(cx));
        sibling.dock.begin_native_shutdown(cx).unwrap();
    });
    settle(cx, &recovery);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn an_explicit_rebuild_replaces_a_failed_generation_without_replaying_its_input(
    cx: &mut TestAppContext,
) {
    let recovery = editor(cx);
    edit_and_move(cx, &recovery);
    let old = current(cx, &recovery);
    let root = cx.update(|cx| recovery.read(cx).roots[&EDITOR]);
    let binding = old.native_binding_for_window(root.window_id()).unwrap();
    let transport = old.native_transport().unwrap();
    cx.update(|cx| {
        // Deliberate protocol fault through the public native ingress, not an
        // OS gesture or an application shortcut for assigning host.failure.
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
    });
    settle(cx, &recovery);
    let failure = old
        .failure()
        .expect("the duplicate press must reject the original input prefix");
    assert_eq!(
        failure.runtime_kind,
        Some(DockspaceRuntimeErrorKind::HostProtocol)
    );
    assert!(old.inspection().unwrap().pending_native_input);
    cx.update(|cx| assert_eq!(recovery.read(cx).phase, Phase::Running));
    assert!(
        !transport.is_frozen(),
        "inspecting a failure never chooses teardown"
    );
    let terminal = cx.hold_window_native_terminal(root);
    let control = cx.update(|cx| recovery.read(cx).control.unwrap());
    let mut visual = VisualTestContext::from_window(control, cx);
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    let rebuild = visual
        .debug_bounds("rebuild-workspace")
        .expect("the recovery control must remain reachable above the diagnostic");
    visual.simulate_click(rebuild.center(), Modifiers::none());
    settle(cx, &recovery);
    assert!(old.native_termination().is_none());
    cx.update(|cx| {
        assert_eq!(
            recovery.read(cx).phase,
            Phase::Retiring(AfterRetirement::Rebuild)
        )
    });
    assert!(terminal.release());
    settle(cx, &recovery);
    assert_eq!(
        old.native_termination(),
        Some(DockNativeTermination::Failed(failure.clone()))
    );
    assert!(!old.native_is_quiescent());
    assert!(
        old.inspection().unwrap().pending_native_input,
        "the failed FIFO stays in its original session"
    );
    assert_eq!(old.failure(), Some(failure));
    let fresh = current(cx, &recovery);
    assert!(fresh.failure().is_none());
    assert!(!fresh.inspection().unwrap().pending_native_input);
    assert_eq!(
        fresh.with_view(|view| view.item(FIRST).unwrap().surface()),
        INSPECTOR
    );
    cx.update(|cx| {
        let owner = recovery.read(cx);
        assert_eq!(owner.phase, Phase::Running);
        assert!(matches!(
            owner.last_termination,
            Some(DockNativeTermination::Failed(_))
        ));
        assert_eq!(owner.workspace.views[0].read(cx).draft, 1);
        assert_eq!(owner.workspace.document.borrow().edits, 1);
        assert_ne!(
            fresh
                .native_binding_for_window(owner.roots[&EDITOR].window_id())
                .unwrap(),
            binding
        );
        recovery.update(cx, |owner, cx| owner.close(cx));
    });
    settle(cx, &recovery);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn partial_reconstruction_retires_its_original_windows_before_an_explicit_retry(
    cx: &mut TestAppContext,
) {
    let recovery = editor(cx);
    edit_and_move(cx, &recovery);
    cx.set_window_open_present_outcomes([
        PlatformWindowPresentOutcome::Submitted,
        PlatformWindowPresentOutcome::Rejected,
    ]);
    cx.update(|cx| recovery.update(cx, |owner, cx| owner.rebuild(cx)));
    settle(cx, &recovery);
    let failed_open = current(cx, &recovery);
    assert_eq!(
        failed_open.native_termination(),
        Some(DockNativeTermination::Drained)
    );
    cx.update(|cx| {
        let owner = recovery.read(cx);
        assert_eq!(owner.phase, Phase::Ready);
        assert!(owner.notice.contains("Opening docking windows failed"));
        assert!(owner.roots.is_empty());
        assert_eq!(cx.windows(), [owner.control.unwrap()]);
        recovery.update(cx, |owner, cx| owner.rebuild(cx));
    });
    settle(cx, &recovery);
    cx.update(|cx| {
        let owner = recovery.read(cx);
        assert_eq!(owner.phase, Phase::Running);
        assert_eq!(owner.roots.len(), 2);
        assert_eq!(owner.workspace.views[0].read(cx).draft, 1);
        assert_eq!(owner.workspace.document.borrow().edits, 1);
        assert_eq!(
            owner
                .workspace
                .dock
                .with_view(|view| view.item(FIRST).unwrap().surface()),
            INSPECTOR
        );
        let windows = [owner.roots[&EDITOR], owner.roots[&INSPECTOR]];
        let [editor, inspector] =
            windows.map(|window| window.update(cx, |_, window, _| window.bounds()).unwrap());
        assert!(editor.origin.x + editor.size.width <= inspector.origin.x);
        recovery.update(cx, |owner, cx| owner.close(cx));
    });
    settle(cx, &recovery);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn closing_the_control_window_during_rebuild_waits_and_cancels_reconstruction(
    cx: &mut TestAppContext,
) {
    let recovery = editor(cx);
    let old = current(cx, &recovery);
    let (control, root) = cx.update(|cx| {
        let owner = recovery.read(cx);
        (owner.control.unwrap(), owner.roots[&EDITOR])
    });
    let terminal = cx.hold_window_native_terminal(root);
    cx.update(|cx| recovery.update(cx, |owner, cx| owner.rebuild(cx)));
    let close = cx.simulate_window_close_request(control);
    assert!(!close.native_close_allowed());
    settle(cx, &recovery);
    assert!(cx.windows().contains(&control));
    cx.update(|cx| {
        assert_eq!(
            recovery.read(cx).phase,
            Phase::Retiring(AfterRetirement::Close)
        )
    });
    assert!(terminal.release());
    settle(cx, &recovery);
    assert!(cx.windows().is_empty());
    assert_eq!(
        old.native_termination(),
        Some(DockNativeTermination::Drained)
    );
}

#[open_gpui::test]
fn app_shutdown_cancels_an_already_queued_rebuild(cx: &mut TestAppContext) {
    let recovery = editor(cx);
    let old = current(cx, &recovery);
    cx.update(|cx| {
        recovery.update(cx, |owner, cx| owner.rebuild(cx));
        cx.shutdown_for_native_exit_test();
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let owner = recovery.read(cx);
        assert_eq!(owner.phase, Phase::Closing);
        assert!(owner.workspace.dock.native_transport().unwrap().is_frozen());
        assert_eq!(
            owner.workspace.dock.native_termination(),
            old.native_termination()
        );
    });
    assert!(cx.windows().is_empty());
}
