//! Original renderer terminals cannot overtake native callback prefixes.

use super::*;
use open_gpui::{PlatformWindowPresentOutcome, RequestFrameOptions, VisualTestContext};

fn frames(visual: &mut VisualTestContext, dock: &DockSurface) {
    for _ in 0..4 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
}

#[open_gpui::test]
fn a_rejected_native_head_retains_its_type_and_the_original_submitted_output(
    cx: &mut TestAppContext,
) {
    use dockspace::runtime::{
        NativeLocalPointerLocation, NativePointerEvent, NativePointerId, NativePointerInput,
        NativePointerOwner,
    };

    cx.defer_next_window_frame_requests();
    let dock = cx.update(workspace);
    let foreign = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    let foreign_window = cx
        .update(|cx| foreign.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    finish(cx, &foreign);
    let foreign_binding = foreign
        .native_binding_for_window(foreign_window.window_id())
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    frames(&mut visual, &dock);
    let original_artifact = dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
        .presented
        .unwrap()
        .1;
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&window.window_id())
        .unwrap()
        .regions
        .get_mut(&FIRST)
        .unwrap()
        .dirty = true;
    frames(&mut visual, &dock);
    let (ticket, transport, bridge) = {
        let mut host = dock.host.borrow_mut();
        let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
        assert!(pending.update.is_none());
        let ticket = pending.ticket.clone();
        let native = host.native.as_mut().unwrap();
        (
            ticket,
            native.transport.clone(),
            native.bridge.take().unwrap(),
        )
    };
    visual.update(|_, cx| {
        transport.pointer(
            NativePointerInput::new_local(
                NativePointerId::new(1),
                NativePointerEvent::Moved,
                NativeLocalPointerLocation::Unknown,
                NativePointerOwner::Native(foreign_binding),
                NativePointerOwner::Unknown,
            ),
            cx,
        );
    });
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted
    );
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    visual.update(|_, cx| dock.pump_native(cx));
    let primary = dock.failure().expect("the invalid provider is rejected");
    assert_eq!(
        primary.runtime_kind,
        Some(DockspaceRuntimeErrorKind::HostProtocol)
    );
    assert_eq!(primary.native_kind, Some(NativeHostErrorKind::InvalidFacts));
    assert!(!transport.is_empty());
    assert_eq!(
        dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
            .presented
            .unwrap()
            .1,
        original_artifact,
        "the terminal cannot publish past a permanently rejected prefix"
    );
    dock.host
        .borrow_mut()
        .fail(&std::io::Error::other("later cleanup observation failed"));
    assert_eq!(dock.failure(), Some(primary.clone()));
    assert_eq!(dock.error(), Some(primary.message));
    visual
        .cx
        .update(|cx| dock.begin_native_shutdown(cx))
        .unwrap();
    visual.run_until_parked();
    visual.cx.update(|cx| dock.pump_native(cx));
    assert!(
        !transport.is_empty(),
        "shutdown must retain the rejected head and affine suffix"
    );
    assert!(!dock.native_is_quiescent());
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted,
        "cleanup cannot relabel the actual renderer terminal"
    );
    visual
        .cx
        .update(|cx| foreign.begin_native_shutdown(cx))
        .unwrap();
    visual.run_until_parked();
    visual.cx.update(|cx| foreign.pump_native(cx));
    assert!(foreign.native_is_quiescent());
}

#[open_gpui::test]
fn a_managed_submission_keeps_its_native_fifo_boundary(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(workspace);
    let first = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let second = cx
        .update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    let windows = [first, second];
    let mut source = VisualTestContext::from_window(windows[0], cx);
    let mut target = VisualTestContext::from_window(windows[1], cx);
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    let old_artifact = dock.host.borrow().mounts[&windows[0].window_id()].regions[&FIRST]
        .presented
        .unwrap()
        .1;
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Deferred);
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&windows[0].window_id())
        .unwrap()
        .regions
        .get_mut(&FIRST)
        .unwrap()
        .dirty = true;
    frames(&mut source, &dock);
    let (ticket, recorded, transport, mut bridge) = {
        let mut host = dock.host.borrow_mut();
        let pending = host.mounts[&windows[0].window_id()]
            .pending
            .as_ref()
            .expect("the source must retain its actual unsubmitted output");
        assert!(pending.update.is_none());
        let ticket = pending.ticket.clone();
        let recorded = host.session.native_input_recorded_count();
        let native = host.native.as_mut().unwrap();
        let transport = native.transport.clone();
        assert!(transport.is_empty());
        (ticket, recorded, transport, native.bridge.take().unwrap())
    };
    source.update(|_, cx| transport.device_removed(cx));
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Submitted);
    assert!(source.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted,
        "the renderer really completed its original submission"
    );
    source.update(|_, cx| transport.device_removed(cx));
    {
        let mut host = dock.host.borrow_mut();
        assert!(
            host.session
                .presented_surface(FIRST)
                .is_some_and(|surface| { surface.matches_presentation_artifact(old_artifact) }),
            "a renderer terminal cannot publish ahead of its older native prefix"
        );
        assert_eq!(host.session.native_input_recorded_count(), recorded);
        bridge.collect(&mut host.session).unwrap();
        assert_eq!(
            host.session.native_input_recorded_count(),
            recorded + 1,
            "collection must stop at the terminal, leaving its later suffix queued"
        );
        assert!(!transport.is_empty());
        host.native.as_mut().unwrap().bridge = Some(bridge);
    }
    source.update(|_, cx| dock.pump_native(cx));
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    assert!(transport.is_empty());
    assert!(dock.host.borrow().session.native_input_recorded_count() >= recorded + 2);
    assert!(
        dock.host.borrow().mounts[&windows[0].window_id()].regions[&FIRST]
            .presented
            .is_some()
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn logical_window_removal_revokes_presented_authority_before_native_terminal(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let anchor = cx
        .update(|cx| dock.open_native_root(SECOND, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    frames(&mut VisualTestContext::from_window(window, cx), &dock);
    frames(&mut VisualTestContext::from_window(anchor, cx), &dock);
    let owner = window.window_id();
    let item = ItemId::new(FIRST.get());
    let roots = dock.with_view(|view| {
        [FIRST, SECOND].map(|surface| view.surface(surface).unwrap().main_root().unwrap().id())
    });
    let (serial, original_artifact, pane) = {
        let host = dock.host.borrow();
        let mount = &host.mounts[&owner];
        let region = &mount.regions[&FIRST];
        assert!(region.packet.as_ref().is_some_and(|packet| {
            !packet.is_deferred() && packet.pane_items().contains(&item)
        }));
        let (serial, artifact) = region
            .presented
            .expect("the original pane must have a real submitted output");
        assert!(
            host.session
                .presented_surface(FIRST)
                .is_some_and(|surface| surface.matches_presentation_artifact(artifact))
        );
        assert!(crate::adapter::input::is_presented(
            &host, owner, FIRST, serial
        ));
        (serial, artifact, host.panels[&item].view.clone())
    };
    let callbacks = dock.native_callbacks_for_window(owner).unwrap();
    // Keep automatic draws out of the logical-close/native-terminal interval.
    cx.skip_drawing();
    let held = cx.hold_window_native_terminal(window);
    dock.take_native_reports();
    cx.update_window(window, |_, window, cx| window.remove_window(cx))
        .unwrap();
    finish(cx, &dock);
    {
        let host = dock.host.borrow();
        assert!(
            host.session.presented_surface(FIRST).is_none(),
            "logical removal must revoke Ready artifact {original_artifact:?} before native terminal"
        );
        assert!(!crate::adapter::input::is_presented(
            &host, owner, FIRST, serial
        ));
        assert!(!host.mounts.contains_key(&owner));
        assert!(!host.owners.contains_key(&FIRST));
        assert!(
            host.unavailable_surfaces.is_empty(),
            "logical close must commit its unavailable contribution before native terminal"
        );
        assert!(!host.presentation_pending);
        let view = host.session.view();
        assert_eq!(view.surfaces().count(), 2);
        for (surface, root) in [FIRST, SECOND].into_iter().zip(roots) {
            let retained = view.surface(surface).unwrap();
            assert_eq!(retained.main_root().unwrap().id(), root);
            assert_eq!(retained.contained().count(), 0);
            let item = view.item(ItemId::new(surface.get())).unwrap();
            assert_eq!(
                (item.surface(), item.root(), item.is_selected()),
                (surface, root, true)
            );
        }
        assert_eq!(host.panels[&item].view.entity_id(), pane.entity_id());
    }
    cx.read(|cx| {
        assert!(!cx.windows().contains(&window));
        assert!(cx.windows().contains(&anchor));
    });
    assert_eq!(
        dock.native_callbacks_for_window(owner).unwrap().binding(),
        callbacks.binding()
    );
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(!dock.native_is_quiescent());
    let reports = dock.take_native_reports();
    assert!(
        reports.iter().any(|report| matches!(report,
            NativeReport::LogicalClosed(binding) if *binding == callbacks.binding()
        )),
        "{reports:?}"
    );
    assert!(
        !reports.iter().any(|report| matches!(report,
            NativeReport::NativeTerminal(binding) if *binding == callbacks.binding()
        )),
        "{reports:?}"
    );

    assert!(held.release());
    finish(cx, &dock);
    let reports = dock.take_native_reports();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::NativeTerminal(binding) if *binding == callbacks.binding()
            ))
            .count(),
        1,
        "the original callback must deliver its real terminal exactly once: {reports:?}"
    );
    callbacks.seal().unwrap();
    assert!(dock.native_callbacks_for_window(owner).is_none());
    assert!(
        dock.host
            .borrow()
            .session
            .retained_native_surface_bindings_for_window(callbacks.binding())
            .next()
            .is_none()
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}
