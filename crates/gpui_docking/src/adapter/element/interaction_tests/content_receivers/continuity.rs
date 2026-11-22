use dockspace::{
    close::CloseDecision,
    model::{DockAnchor, DockPlacement},
};
use open_gpui::{
    Empty, PlatformFrameDeliveryMode, PlatformInput, PointerCancelEvent, PointerCancelReason,
    WindowFrameSubmissionOutcome,
};

use super::*;

#[open_gpui::test]
fn a_siblings_unaccepted_update_preserves_the_presented_editor(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling_surface = SurfaceId::new(52);
    for managed in [false, true] {
        let (dock, editors) = content_fixture_regions(cx, managed, true);
        cx.defer_next_window_frame_requests();
        let mut source = open_content(cx, &dock, managed);
        let source_window = source.update(|window, _| window.window_handle());
        cx.defer_next_window_frame_requests();
        let sibling_window = if managed {
            cx.update(|cx| dock.open_native_root(sibling_surface, false, Default::default(), cx))
                .unwrap()
        } else {
            cx.open_window(size(px(800.0), px(600.0)), |_, _| {
                super::super::wheel::SingleRegion(dock.clone(), sibling_surface)
            })
            .into()
        };
        let mut sibling = VisualTestContext::from_window(sibling_window, cx);
        frames(&mut sibling, &dock);
        let mut source = active_visual(source_window, cx);
        frames(&mut source, &dock);
        let editor = &editors[0];
        let focus = editor.read_with(&source, |editor, _| editor.focus.clone());
        let focus_claim = source.update(|window, cx| {
            assert!(window.is_focus_handle_rendered(&focus));
            window.focus(&focus, cx);
            window.focus_claim_revision()
        });
        source.simulate_keystrokes("p");
        assert_eq!(
            editor.read_with(&source, |editor, _| editor.text.clone()),
            "p"
        );

        cx.set_window_frame_delivery_mode(sibling_window, PlatformFrameDeliveryMode::Deferred);
        cx.simulate_window_resize(sibling_window, size(px(760.0), px(510.0)));
        assert!(sibling.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        let original = cx.pending_window_frame_submission(sibling_window).unwrap();
        assert!(dock.is_update_pending());
        for key in ["x", "y"] {
            frames(&mut source, &dock);
            assert!(dock.host.borrow().mounts[&source_window.window_id()].waiting_for_update);
            source.update(|window, _| {
                assert!(window.is_focus_handle_rendered(&focus));
                assert!(focus.is_focused(window));
                assert_eq!(window.focus_claim_revision(), focus_claim);
            });
            source.simulate_keystrokes(key);
        }
        assert_eq!(
            editor.read_with(&source, |editor, _| editor.text.clone()),
            "pxy"
        );
        assert!(dock.is_update_pending());
        cx.update(|_| assert!(original.complete(PlatformWindowPresentOutcome::Submitted)));
        cx.set_window_frame_delivery_mode(sibling_window, PlatformFrameDeliveryMode::Synchronous);
        frames(&mut sibling, &dock);
        for _ in 0..6 {
            sibling.simulate_scheduled_frame();
            source.simulate_scheduled_frame();
        }
        source.simulate_keystrokes("z");
        assert_eq!(
            editor.read_with(&source, |editor, _| editor.text.clone()),
            "pxyz",
            "managed={managed}"
        );
        assert!(source.update(|window, _| focus.is_focused(window)));
        if managed {
            shutdown(cx, &dock, true);
        } else {
            sibling.update(|window, cx| window.remove_window(cx));
            source.update(|window, cx| window.remove_window(cx));
        }
    }
}

#[open_gpui::test]
fn native_tab_routing_uses_the_presented_regions_transform(cx: &mut TestAppContext) {
    use open_gpui::{SubtreeTransform, SubtreeTransformExt, SubtreeTransformOrigin};

    struct TransformedDock {
        dock: DockSurface,
        transform: Rc<Cell<SubtreeTransform>>,
    }

    impl Render for TransformedDock {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                div()
                    .w(px(500.0))
                    .h(px(260.0))
                    .child(self.dock.element(SURFACE))
                    .with_subtree_transform(self.transform.get()),
            )
        }
    }

    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let (dock, _) = super::super::fixture_with_mode(cx, false, true);
    let transform = Rc::new(Cell::new(SubtreeTransform::identity()));
    cx.defer_next_window_frame_requests();
    let window = cx.open_window(size(px(900.0), px(480.0)), |window, cx| {
        dock.bind_native_root(SURFACE, true, window, cx).unwrap();
        TransformedDock {
            dock: dock.clone(),
            transform: transform.clone(),
        }
    });
    let mut visual = active_visual(window.into(), cx);
    cx.set_platform_hovered_window(Some(window.into()));
    prepare_native_input(&mut visual);
    frames(&mut visual, &dock);
    let tab = format!("dockspace:tab:{}", SECOND.get());
    let original = visual.debug_bounds(&tab).unwrap().center();
    for (scale, translation) in [(1.0, px(124.0)), (1.5, px(0.0))] {
        transform.set(
            SubtreeTransform::try_new(
                size(scale, scale),
                point(translation, px(0.0)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        );
        frames(&mut visual, &dock);
        let displayed = visual.debug_bounds(&tab).unwrap().center();
        assert_eq!(displayed, original * scale + point(translation, px(0.0)));
        press(&mut visual, displayed, true);
        frames(&mut visual, &dock);
        release_drag(&mut visual, displayed, true);
        frames(&mut visual, &dock);
        assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    frame.select_item_current(FIRST)?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    }
    shutdown(cx, &dock, true);
}

#[open_gpui::test]
fn sizing_metadata_keeps_an_unchanged_editor_focused(cx: &mut TestAppContext) {
    use dockspace::geometry::PaneSizing;

    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    for managed in [false, true] {
        let (dock, editors) = content_fixture(cx, managed);
        let editor = &editors[0];
        let mut visual = open_content(cx, &dock, managed);
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    frame.dock_item_current(SECOND, DockPlacement::After(FIRST))?;
                    frame.select_item_current(FIRST)?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        let focus = editor.read_with(&visual, |editor, _| editor.focus.clone());
        let original = pane_center(&dock, &mut visual, FIRST);
        visual.update(|window, cx| window.focus(&focus, cx));
        visual.simulate_keystrokes("x");
        for (item, sizing, key) in [
            (
                FIRST,
                PaneSizing::builder()
                    .min_width(120.0)
                    .min_height(90.0)
                    .preferred_width(240.0)
                    .build()
                    .unwrap(),
                "y",
            ),
            (
                SECOND,
                PaneSizing::builder()
                    .min_width(121.0)
                    .min_height(90.0)
                    .build()
                    .unwrap(),
                "z",
            ),
        ] {
            visual.update(|_, cx| {
                dock.update_pane_metadata(item, format!("Pane {}", item.get()), sizing, cx)
                    .unwrap();
            });
            frames(&mut visual, &dock);
            assert_eq!(pane_center(&dock, &mut visual, FIRST), original);
            assert!(
                visual.update(|window, _| focus.is_focused(window)),
                "managed={managed}, item={item:?}"
            );
            visual.simulate_keystrokes(key);
        }
        assert_eq!(
            editor.read_with(&visual, |editor, _| editor.text.clone()),
            "xyz"
        );
        if managed {
            shutdown(cx, &dock, true);
        } else {
            visual.update(|window, cx| window.remove_window(cx));
        }
    }
}

#[open_gpui::test]
fn a_siblings_unaccepted_update_preserves_captured_drag_terminals(cx: &mut TestAppContext) {
    for cancel in [false, true] {
        let (dock, _) = content_fixture_regions(cx, false, true);
        cx.defer_next_window_frame_requests();
        let mut source = open_content(cx, &dock, false);
        let source_window = source.update(|window, _| window.window_handle());
        cx.defer_next_window_frame_requests();
        let sibling_window = cx.open_window(size(px(800.0), px(600.0)), |_, _| {
            super::super::wheel::SingleRegion(dock.clone(), SurfaceId::new(52))
        });
        let mut sibling = VisualTestContext::from_window(sibling_window.into(), cx);
        frames(&mut sibling, &dock);
        let mut source = active_visual(source_window, cx);
        cx.set_platform_hovered_window(Some(source_window));
        frames(&mut source, &dock);
        let capture = begin_drag(&dock, &mut source, FIRST);

        cx.set_window_frame_delivery_mode(
            sibling_window.into(),
            PlatformFrameDeliveryMode::Deferred,
        );
        cx.simulate_window_resize(sibling_window.into(), size(px(760.0), px(510.0)));
        assert!(sibling.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        let pending = cx
            .pending_window_frame_submission(sibling_window.into())
            .unwrap();
        frames(&mut source, &dock);
        assert!(dock.host.borrow().mounts[&source_window.window_id()].waiting_for_update);
        let position = source.update(|window, _| {
            assert_eq!(window.captured_pointer(), Some(capture));
            window.mouse_position()
        });

        if cancel {
            source.update(|window, cx| {
                window.dispatch_event(
                    PlatformInput::PointerCanceled(PointerCancelEvent {
                        reason: PointerCancelReason::CaptureRevoked,
                    }),
                    cx,
                );
            });
        } else {
            source.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        }
        assert_eq!(dock.host.borrow().inputs.len(), 1, "cancel: {cancel}");
        assert!(dock.has_active_gesture());
        assert!(source.update(|window, _| window.captured_pointer().is_none()));
        cx.update(|_| assert!(pending.complete(PlatformWindowPresentOutcome::Submitted)));
        cx.set_window_frame_delivery_mode(
            sibling_window.into(),
            PlatformFrameDeliveryMode::Synchronous,
        );
        frames(&mut sibling, &dock);
        frames(&mut source, &dock);
        assert!(!dock.has_active_gesture());
        assert!(dock.host.borrow().inputs.is_empty());
        assert_eq!(
            dock.with_view(|view| view.item(FIRST).unwrap().surface()),
            SURFACE
        );
        sibling.update(|window, cx| window.remove_window(cx));
        source.update(|window, cx| window.remove_window(cx));
    }
}

#[open_gpui::test]
fn resized_editor_finishes_its_original_press_while_a_sibling_waits(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling_surface = SurfaceId::new(52);
    for managed in [false, true] {
        for cancel in [false, true] {
            let (dock, editors) = content_fixture_regions(cx, managed, true);
            cx.defer_next_window_frame_requests();
            let mut source = open_content(cx, &dock, managed);
            let source_window = source.update(|window, _| window.window_handle());
            cx.defer_next_window_frame_requests();
            let sibling_window = if managed {
                cx.update(|cx| {
                    dock.open_native_root(sibling_surface, false, Default::default(), cx)
                })
                .unwrap()
            } else {
                cx.open_window(size(px(620.0), px(400.0)), |_, _| {
                    super::super::wheel::SingleRegion(dock.clone(), sibling_surface)
                })
                .into()
            };
            let mut sibling = VisualTestContext::from_window(sibling_window, cx);
            frames(&mut sibling, &dock);
            let mut source = active_visual(source_window, cx);
            cx.set_platform_hovered_window(Some(source_window));
            frames(&mut source, &dock);
            let content = pane_center(&dock, &mut source, FIRST);
            press(&mut source, content, managed);
            frames(&mut source, &dock);
            assert!(
                dock.host.borrow().mounts[&source_window.window_id()]
                    .content_press
                    .is_some()
            );

            cx.set_window_frame_delivery_mode(sibling_window, PlatformFrameDeliveryMode::Deferred);
            cx.simulate_window_resize(sibling_window, size(px(680.0), px(470.0)));
            assert!(sibling.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            let pending = cx.pending_window_frame_submission(sibling_window).unwrap();
            cx.simulate_window_resize(source_window, size(px(760.0), px(510.0)));
            frames(&mut source, &dock);
            let host = dock.host.borrow();
            let mount = &host.mounts[&source_window.window_id()];
            assert!(mount.waiting_for_update && mount.input_frames.is_empty());
            drop(host);
            if cancel {
                if managed {
                    source.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
                }
                source.simulate_event(PointerCancelEvent {
                    reason: PointerCancelReason::CaptureRevoked,
                });
            } else {
                if managed {
                    source.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
                }
                source.simulate_mouse_up(content, MouseButton::Left, Modifiers::none());
            }
            assert!(
                dock.host.borrow().mounts[&source_window.window_id()]
                    .content_press
                    .is_none()
            );
            cx.update(|_| assert!(pending.complete(PlatformWindowPresentOutcome::Submitted)));
            cx.set_window_frame_delivery_mode(
                sibling_window,
                PlatformFrameDeliveryMode::Synchronous,
            );
            frames(&mut sibling, &dock);
            frames(&mut source, &dock);

            let clicks = editors[0].read_with(&source, |editor, _| editor.clicks);
            if managed {
                prepare_native_input(&mut source);
                frames(&mut source, &dock);
            }
            let content = pane_center(&dock, &mut source, FIRST);
            press(&mut source, content, managed);
            release_drag(&mut source, content, managed);
            frames(&mut source, &dock);
            assert_eq!(
                editors[0].read_with(&source, |editor, _| editor.clicks),
                clicks + 1
            );
            if managed {
                shutdown(cx, &dock, true);
            } else {
                sibling.update(|window, cx| window.remove_window(cx));
                source.update(|window, cx| window.remove_window(cx));
            }
        }
    }
}

fn press(visual: &mut VisualTestContext, position: Point<Pixels>, managed: bool) {
    if managed {
        visual.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
        native_input(
            visual,
            position,
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
        );
    } else {
        visual.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
    }
}

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
        (
            region.presented_panes.as_ref().unwrap().geometry[&FIRST].clone(),
            artifact,
        )
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
        assert!(mount.regions[&SURFACE].presented_panes.is_none());
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
