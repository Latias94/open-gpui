//! An unsubmitted destination cannot retire the source's native window or View.

use super::*;
use dockspace::{
    geometry::{PhysicalPoint, PhysicalRect},
    model::NativeWindowPlacement,
    runtime::{
        DockspaceManipulationCancelReason, DockspaceManipulationFeedback,
        DockspaceManipulationStatus, NativeDesktopPointerLocation, NativeDesktopPosition,
        NativeGlobalFocus, NativePointerButton, NativePointerCancelReason, NativePointerEvent,
        NativePointerHover, NativePointerId, NativePointerInput, NativePointerOwner,
        PointerDeviceKind, PointerEventMetadata,
    },
};
use open_gpui::{AnyWindowHandle, Point, WindowMutationDomain, point};

fn pointer(
    cx: &mut TestAppContext,
    dock: &DockSurface,
    source: AnyWindowHandle,
    target: AnyWindowHandle,
    position: Point<open_gpui::Pixels>,
    event: NativePointerEvent,
) -> Vec<DockspaceManipulationFeedback> {
    let source = dock.native_binding_for_window(source.window_id()).unwrap();
    let target_binding = dock.native_binding_for_window(target.window_id()).unwrap();
    let desktop = cx
        .update_window(target, |_, window, _| {
            let facts = window.platform_facts();
            let geometry = facts.physical_geometry.unwrap();
            PhysicalPoint::new(
                f64::from(geometry.client_bounds().origin.x.0)
                    + f64::from(f32::from(position.x)) * f64::from(facts.scale_factor),
                f64::from(geometry.client_bounds().origin.y.0)
                    + f64::from(f32::from(position.y)) * f64::from(facts.scale_factor),
            )
            .unwrap()
        })
        .unwrap();
    let metadata =
        PointerEventMetadata::new(PointerDeviceKind::Mouse, dock.host.borrow().observed_at());
    let feedback = cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        host.session
            .record_native_pointer(
                NativePointerInput::new(
                    NativePointerId::new(1),
                    event,
                    NativeDesktopPointerLocation::new(
                        NativeDesktopPosition::Exact(desktop),
                        NativePointerHover::Dock(target_binding),
                        None,
                    ),
                    NativePointerOwner::Native(source),
                    if matches!(event, NativePointerEvent::ButtonReleased(_)) {
                        NativePointerOwner::None
                    } else {
                        NativePointerOwner::Native(source)
                    },
                )
                .with_metadata(metadata),
            )
            .unwrap();
        let super::super::host::HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let report =
            super::super::host_native::begin_frame(session, mounts, native.as_ref(), None, cx)
                .unwrap()
                .commit()
                .unwrap();
        let feedback = report.manipulation_feedback().to_vec();
        host.apply_report(report);
        drop(host);
        cx.refresh_windows();
        feedback
    });
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    feedback
}

#[open_gpui::test]
fn an_unsubmitted_target_cancels_release_and_preserves_the_native_child(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, views, document) = managed_workspace(cx);
    let root = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(FIRST, true, window, cx).unwrap();
        Root {
            dock: dock.clone(),
            surfaces: vec![FIRST],
        }
    });
    cx.update_window(root.into(), |_, window, _| {
        let placement = window
            .physical_placement_for_outer_bounds(open_gpui::Bounds::new(
                point(open_gpui::DevicePixels(0), open_gpui::DevicePixels(0)),
                size(open_gpui::DevicePixels(800), open_gpui::DevicePixels(600)),
            ))
            .unwrap();
        assert!(matches!(
            window.request_physical_window_placement(placement),
            open_gpui::WindowMutationDispatch::Queued(_)
        ));
    })
    .unwrap();
    assert!(cx.flush_window_mutation(root.into(), WindowMutationDomain::Placement));
    frames(&mut VisualTestContext::from_window(root.into(), cx), &dock);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.create_native_surface_current(
                    SECOND,
                    FIRST,
                    NativeWindowPlacement::new(PhysicalRect::new(900.0, 0.0, 600.0, 450.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();
    let child = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != root.into())
            .unwrap()
    });
    assert!(cx.flush_window_mutation(child, WindowMutationDomain::Placement));
    frames(&mut VisualTestContext::from_window(child, cx), &dock);
    frames(&mut VisualTestContext::from_window(root.into(), cx), &dock);
    assert_painted_pane(&dock, child, SECOND, B);
    let original_view = views[1].entity_id();
    let source = VisualTestContext::from_window(child, cx)
        .debug_bounds("dockspace:tab:22")
        .unwrap()
        .center();
    let target = point(px(400.0), px(300.0));
    let child_binding = dock.native_binding_for_window(child.window_id()).unwrap();
    cx.update(|cx| {
        dock.native_transport()
            .unwrap()
            .focus(NativeGlobalFocus::Dock(child_binding), cx);
    });
    pointer(
        cx,
        &dock,
        child,
        child,
        source,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    frames(&mut VisualTestContext::from_window(child, cx), &dock);
    pointer(
        cx,
        &dock,
        child,
        root.into(),
        target,
        NativePointerEvent::Moved,
    );
    assert!(dock.has_active_gesture());
    let root_binding = dock.native_binding_for_window(root.window_id()).unwrap();
    cx.update(|cx| {
        dock.native_transport()
            .unwrap()
            .focus(NativeGlobalFocus::Dock(root_binding), cx);
    });
    let mut destination = VisualTestContext::from_window(root.into(), cx);
    frames(&mut destination, &dock);
    pointer(
        cx,
        &dock,
        child,
        root.into(),
        target,
        NativePointerEvent::Moved,
    );
    destination.set_window_present_outcome(root.into(), PlatformWindowPresentOutcome::Deferred);
    // Force a new core output so the control cannot pass by replaying an already submitted packet.
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&root.window_id())
        .unwrap()
        .regions
        .get_mut(&FIRST)
        .unwrap()
        .dirty = true;
    frames(&mut destination, &dock);
    {
        let host = dock.host.borrow();
        let pending = host.mounts[&root.window_id()].pending.as_ref().unwrap();
        assert!(
            pending.update.is_none(),
            "the accepted frame released its update lease"
        );
        let snapshot = pending.ticket.snapshot();
        assert!(snapshot.accepted_frame_generation().is_some());
        assert!(snapshot.submitted_frame_generation().is_none());
    }
    let feedback = pointer(
        cx,
        &dock,
        child,
        root.into(),
        target,
        NativePointerEvent::ButtonReleased(NativePointerButton::Primary),
    );
    assert!(
        matches!(
            feedback.as_slice(),
            [feedback] if feedback.status() == DockspaceManipulationStatus::Cancelled(
                DockspaceManipulationCancelReason::UnknownTargetAuthority
            )
        ),
        "{feedback:?}"
    );
    assert!(!dock.host.borrow().session.has_pending_presentation());
    assert!(!dock.has_active_gesture());
    cx.update_window(root.into(), |_, window, cx| {
        super::super::presentation::discard(&dock.host, window, cx);
    })
    .unwrap();
    assert!(
        !dock.host.borrow().session.has_pending_presentation(),
        "retiring the unsubmitted outputs cannot revive the cancelled release"
    );
    assert_eq!(
        dock.with_view(|view| view.item(B).unwrap().surface()),
        SECOND
    );
    assert_eq!(views[1].entity_id(), original_view);
    assert_eq!(document.borrow().text, "shared");
    assert_eq!(
        dock.native_binding_for_window(child.window_id()),
        Some(child_binding)
    );
    assert!(cx.update(|cx| cx.windows().contains(&child)));
    destination.set_window_present_outcome(root.into(), PlatformWindowPresentOutcome::Submitted);
    frames(&mut destination, &dock);
    frames(&mut VisualTestContext::from_window(child, cx), &dock);
    let mut source_window = VisualTestContext::from_window(child, cx);
    let source = source_window
        .debug_bounds("dockspace:tab:22")
        .unwrap()
        .center();
    let moved = point(source.x + px(40.0), source.y + px(10.0));
    cx.update(|cx| {
        dock.native_transport()
            .unwrap()
            .focus(NativeGlobalFocus::Dock(child_binding), cx);
    });
    cx.run_until_parked();
    pointer(
        cx,
        &dock,
        child,
        child,
        source,
        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
    );
    pointer(cx, &dock, child, child, moved, NativePointerEvent::Moved);
    assert!(
        dock.has_active_gesture(),
        "a cancelled transfer cannot block the next drag"
    );
    pointer(
        cx,
        &dock,
        child,
        child,
        moved,
        NativePointerEvent::StreamCancelled(
            NativePointerCancelReason::ExplicitPlatformCancellation,
        ),
    );
    assert!(!dock.has_active_gesture());
    frames(&mut source_window, &dock);
    let edit = source_window.debug_bounds("view:22").unwrap();
    source_window.simulate_click(edit.center(), Modifiers::default());
    frames(&mut source_window, &dock);
    assert_eq!(views[1].read_with(&source_window, |view, _| view.clicks), 1);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}
