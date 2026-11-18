use dockspace::{
    close::CloseDecision,
    model::{DockAnchor, DockPlacement},
};
use open_gpui::{Empty, WindowFrameSubmissionOutcome};

use super::*;

#[open_gpui::test]
fn reused_surface_root_and_item_wait_for_their_original_output_despite_identical_geometry(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let root = RootId::new(71);
    let (dock, editor) = cx.update(|cx| {
        let tag = HitboxTag::new();
        let editor = cx.new(|cx| EditorPane {
            focus: cx.focus_handle(),
            tag,
            declared: true,
            blocker: false,
            popup: false,
            text: String::new(),
            clicks: 0,
            scrolls: Rc::default(),
        });
        let anchor = cx.new(|_| Empty);
        let panes = [
            DockPane::new(FIRST, "Editor", editor.clone())
                .with_focus_handle(editor.read(cx).focus.clone())
                .with_content_receiver(tag),
            DockPane::new(SECOND, "Anchor", anchor),
        ];
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                SURFACE,
                DockspaceRootLayout::new(root, DockspaceNode::central_tabs([FIRST])),
            ),
            DockspaceSurfaceLayout::new(
                SurfaceId::new(52),
                DockspaceRootLayout::new(RootId::new(72), DockspaceNode::tabs([SECOND])),
            ),
        ])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, editor)
    });
    let mut visual = open_content(cx, &dock, false);
    let window = visual.update(|window, _| window.window_handle());
    let owner = window.window_id();
    let focus = editor.read_with(&visual, |editor, _| editor.focus.clone());
    let content = pane_center(&dock, &mut visual, FIRST);
    visual.simulate_click(content, Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(editor.read_with(&visual, |editor, _| editor.clicks), 1);
    assert!(visual.update(|window, cx| focus.contains_focused(window, cx)));
    let (original_geometry, original_artifact) = visual.update(|window, _| {
        let host = dock.host.borrow();
        let region = &host.mounts[&owner].regions[&SURFACE];
        let (serial, artifact) = region.presented.unwrap();
        assert!(input::is_presented(&host, owner, SURFACE, serial));
        assert!(
            host.session
                .presented_surface(SURFACE)
                .is_some_and(|surface| surface.matches_presentation_artifact(artifact))
        );
        assert!(
            host.mounts[&owner].input_frames[0]
                .content_hitboxes
                .iter()
                .any(|hitbox| hitbox.window_point_target_rank(content, window) == Some(0))
        );
        (region.presented_panes[&FIRST].clone(), artifact)
    });

    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.request_close_item_current(FIRST)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let plans = dock.pending_content_close_plans().unwrap();
        assert_eq!(plans.len(), 1);
        dock.update(
            |frame| {
                let decisions = frame
                    .pending_close_decisions(plans[0].request())
                    .collect::<Vec<_>>();
                assert_eq!(decisions.len(), 1);
                for decision in decisions {
                    frame.resolve_close(
                        plans[0].request(),
                        decision.token(),
                        CloseDecision::Allow,
                    )?;
                }
                Ok(())
            },
            cx,
        )
        .unwrap();
        assert!(dock.with_view(|view| view.item(FIRST).is_none()));
        dock.update(
            |frame| {
                frame.retire_empty_surface_current(SURFACE)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        dock.with_view(|view| {
            assert!(view.surface(SURFACE).is_none());
            assert!(view.root(root).is_none());
        });
    });
    frames(&mut visual, &dock);
    visual.update(|window, cx| {
        let host = dock.host.borrow();
        let mount = &host.mounts[&owner];
        assert!(mount.pending.is_none());
        assert!(host.session.presented_surface(SURFACE).is_none());
        assert!(mount.regions[&SURFACE].presented_panes.is_empty());
        assert!(
            mount.regions[&SURFACE]
                .packet
                .as_ref()
                .unwrap()
                .pane_items()
                .is_empty()
        );
        assert!(mount.input_frames[0].panes.is_empty());
        assert!(!window.is_focus_handle_rendered(&focus));
        assert!(!focus.contains_focused(window, cx));
    });

    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.attach_empty_surface_current(SURFACE, root)?;
                frame.open_item_current(FIRST, DockPlacement::Center(DockAnchor::Central(root)))?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        dock.with_view(|view| {
            assert_eq!(
                view.surface(SURFACE).unwrap().main_root().unwrap().id(),
                root
            );
            let item = view.item(FIRST).unwrap();
            assert_eq!(item.surface(), SURFACE);
            assert_eq!(item.root(), root);
            assert!(item.is_active());
        });
    });
    frames(&mut visual, &dock);
    let (ticket, serial) = visual.update(|window, cx| {
        let host = dock.host.borrow();
        let pending = host.mounts[&owner].pending.as_ref().unwrap();
        assert!(pending.update.is_none());
        let frame = pending
            .accepted_input_frames()
            .unwrap()
            .iter()
            .find(|frame| frame.surface == SURFACE)
            .unwrap();
        let (_, geometry) = frame.panes.iter().find(|(item, _)| *item == FIRST).unwrap();
        assert_eq!(geometry, &original_geometry);
        assert!(frame.content_hitboxes.is_empty());
        assert!(!input::is_presented(&host, owner, SURFACE, pending.serial));
        assert!(host.session.presented_surface(SURFACE).is_none());
        assert_eq!(
            host.session.pane_focus_request(SURFACE).unwrap().item(),
            Some(FIRST)
        );
        assert!(!window.is_focus_handle_rendered(&focus));
        assert!(!focus.contains_focused(window, cx));
        (pending.ticket.clone(), pending.serial)
    });
    let deferred = ticket.snapshot();
    assert_eq!(deferred.outcome(), WindowFrameSubmissionOutcome::Accepted);
    assert!(deferred.accepted_frame_generation().is_some());
    assert_eq!(deferred.submitted_frame_generation(), None);

    visual.simulate_click(content, Modifiers::none());
    visual.simulate_keystrokes("x");
    frames(&mut visual, &dock);
    assert_eq!(editor.read_with(&visual, |editor, _| editor.clicks), 1);
    assert!(editor.read_with(&visual, |editor, _| editor.text.is_empty()));
    visual.update(|window, cx| {
        let host = dock.host.borrow();
        let pending = host.mounts[&owner].pending.as_ref().unwrap();
        assert!(pending.ticket.same_authority(&ticket));
        assert_eq!(pending.serial, serial);
        assert!(host.session.presented_surface(SURFACE).is_none());
        assert!(!window.is_focus_handle_rendered(&focus));
        assert!(!focus.contains_focused(window, cx));
    });
    let deferred = ticket.snapshot();
    assert_eq!(deferred.outcome(), WindowFrameSubmissionOutcome::Accepted);
    let accepted_generation = deferred.accepted_frame_generation().unwrap();
    assert_eq!(deferred.submitted_frame_generation(), None);

    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    let submitted = ticket.snapshot();
    assert_eq!(submitted.outcome(), WindowFrameSubmissionOutcome::Submitted);
    assert_eq!(
        submitted.accepted_frame_generation(),
        Some(accepted_generation)
    );
    assert_eq!(
        submitted.submitted_frame_generation(),
        Some(accepted_generation)
    );
    {
        let host = dock.host.borrow();
        let (presented_serial, artifact) = host.mounts[&owner].regions[&SURFACE].presented.unwrap();
        assert_eq!(presented_serial, serial);
        assert_ne!(artifact, original_artifact);
        assert!(
            host.session
                .presented_surface(SURFACE)
                .is_some_and(|surface| surface.matches_presentation_artifact(artifact))
        );
    }
    frames(&mut visual, &dock);
    assert!(visual.update(|window, _| window.is_focus_handle_rendered(&focus)));
    assert!(visual.update(|window, cx| focus.contains_focused(window, cx)));
    assert!(
        dock.host
            .borrow()
            .session
            .pane_focus_request(SURFACE)
            .is_none()
    );
    visual.simulate_click(content, Modifiers::none());
    frames(&mut visual, &dock);
    assert_eq!(editor.read_with(&visual, |editor, _| editor.clicks), 2);
}
