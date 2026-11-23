use dockspace::{
    close::CloseDecision,
    model::{DockAnchor, DockPlacement},
};
use open_gpui::{
    Empty, PlatformFrameDeliveryMode, PlatformInput, PointerCancelEvent, PointerCancelReason,
    SubtreeTransform, SubtreeTransformExt, SubtreeTransformOrigin, WindowFrameSubmissionOutcome,
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
                    .primary_press
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
                    .primary_press
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
fn native_content_terminals_follow_a_continued_root_across_layout_reset(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    for cancel in [false, true] {
        for repaint_before_terminal in [false, true] {
            let (dock, _) = content_fixture(cx, true);
            let mut visual = open_content(cx, &dock, true);
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
            let content = pane_center(&dock, &mut visual, FIRST);
            press(&mut visual, content, true);
            frames(&mut visual, &dock);
            let owner = visual.update(|window, _| window.window_handle().window_id());
            let original = dock.native_binding_for_window(owner).unwrap();
            let roots = {
                let host = dock.host.borrow();
                assert!(host.mounts[&owner].primary_press.is_some());
                host.session
                    .native_surface_bindings_for_window(original)
                    .map(|binding| (binding, binding.surface()))
                    .collect::<Vec<_>>()
            };
            let reset = dock
                .host
                .borrow()
                .session
                .prepare_layout_reset(dock.with_view(|view| view.to_layout()))
                .unwrap();
            visual.update(|_, cx| {
                dock.update(
                    |frame| {
                        frame.submit_native_layout_reset_with_roots(&reset, roots)?;
                        Ok(())
                    },
                    cx,
                )
                .unwrap();
            });
            assert_ne!(dock.native_binding_for_window(owner), Some(original));
            if repaint_before_terminal {
                frames(&mut visual, &dock);
            }
            if cancel {
                visual.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
                visual.simulate_event(PointerCancelEvent {
                    reason: PointerCancelReason::CaptureRevoked,
                });
            } else {
                release_drag(&mut visual, content, true);
            }
            frames(&mut visual, &dock);
            let next = visual
                .debug_bounds(&format!("dockspace:tab:{}", SECOND.get()))
                .unwrap()
                .center();
            press(&mut visual, next, true);
            release_drag(&mut visual, next, true);
            frames(&mut visual, &dock);
            assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
            shutdown(cx, &dock, true);
        }
    }
}

#[open_gpui::test]
fn transformed_content_keeps_keyboard_focus_while_its_geometry_changes(cx: &mut TestAppContext) {
    struct TransformedContent {
        dock: DockSurface,
        transform: Rc<Cell<SubtreeTransform>>,
    }

    impl Render for TransformedContent {
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
    for managed in [false, true] {
        let (dock, editors) = content_fixture(cx, managed);
        let transform = Rc::new(Cell::new(SubtreeTransform::identity()));
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(900.0), px(480.0)), |window, cx| {
            if managed {
                dock.bind_native_root(SURFACE, true, window, cx).unwrap();
            }
            TransformedContent {
                dock: dock.clone(),
                transform: transform.clone(),
            }
        });
        let mut visual = active_visual(window.into(), cx);
        cx.set_platform_hovered_window(Some(window.into()));
        if managed {
            prepare_native_input(&mut visual);
        }
        frames(&mut visual, &dock);
        let focus = editors[0].read_with(&visual, |editor, _| editor.focus.clone());
        let claim = visual.update(|window, cx| {
            window.focus(&focus, cx);
            window.focus_claim_revision()
        });
        for (scale, translation, key) in [(1.0, 0.0, "x"), (1.0, 16.0, "y"), (0.875, 0.0, "z")] {
            transform.set(
                SubtreeTransform::try_new(
                    size(scale, scale),
                    point(px(translation), px(0.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            );
            frames(&mut visual, &dock);
            assert!(visual.update(|window, _| focus.is_focused(window)));
            assert_eq!(
                visual.update(|window, _| window.focus_claim_revision()),
                claim
            );
            visual.simulate_keystrokes(key);
        }
        assert_eq!(
            editors[0].read_with(&visual, |editor, _| editor.text.clone()),
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
fn content_focus_waits_without_overriding_a_newer_window_focus_claim(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
    let sibling_surface = SurfaceId::new(52);
    let sibling_item = ItemId::new(64);
    for managed in [false, true] {
        for (move_pane, blur, click) in [
            (false, false, false),
            (false, true, false),
            (false, false, true),
            (true, false, false),
        ] {
            let source_content = DockspaceNode::equal_split(
                DockspaceAxis::Horizontal,
                [DockspaceNode::tabs([FIRST]), DockspaceNode::tabs([SECOND])],
            )
            .unwrap();
            let surfaces = [
                (SURFACE, source_content),
                (sibling_surface, DockspaceNode::tabs([sibling_item])),
            ]
            .map(|(surface, content)| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(RootId::new(surface.get()), content),
                )
            });
            let (dock, editors) =
                content_fixture_layout(cx, managed, surfaces, &[FIRST, SECOND, sibling_item]);
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
            let focus = editors[0].read_with(&source, |editor, _| editor.focus.clone());
            let destination = editors[1].read_with(&source, |editor, _| editor.focus.clone());
            let old_content = pane_center(&dock, &mut source, FIRST);
            source.update(|window, cx| window.focus(&focus, cx));
            source.simulate_keystrokes("a");

            cx.set_window_frame_delivery_mode(source_window, PlatformFrameDeliveryMode::Deferred);
            assert!(source.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            let source_output = cx.pending_window_frame_submission(source_window).unwrap();
            assert!(!dock.is_update_pending());
            cx.set_window_frame_delivery_mode(sibling_window, PlatformFrameDeliveryMode::Deferred);
            source.update(|_, cx| {
                dock.update(
                    |frame| {
                        if move_pane {
                            frame.dock_item_current(FIRST, DockPlacement::After(SECOND))?;
                        } else {
                            frame.select_item_current(SECOND)?;
                        }
                        Ok(())
                    },
                    cx,
                )
                .unwrap();
                assert_eq!(
                    dock.host
                        .borrow()
                        .session
                        .pane_focus_request(SURFACE)
                        .and_then(|request| request.item()),
                    Some(if move_pane { FIRST } else { SECOND }),
                );
            });

            cx.simulate_window_resize(sibling_window, size(px(642.0), px(412.0)));
            assert!(sibling.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            let pending = cx.pending_window_frame_submission(sibling_window).unwrap();
            assert!(
                dock.is_update_pending(),
                "managed={managed}, moved={move_pane}, blur={blur}, click={click}, mounts={:?}",
                dock.inspection().unwrap().mounts,
            );
            cx.update(|_| {
                if !source_output.complete(PlatformWindowPresentOutcome::Submitted) {
                    assert_eq!(
                        source_output.status(),
                        open_gpui::PlatformFrameSubmissionStatus::Cancelled,
                    );
                }
            });
            cx.set_window_frame_delivery_mode(
                source_window,
                PlatformFrameDeliveryMode::Synchronous,
            );
            if blur {
                source.update(|window, cx| window.blur(cx));
            }
            frames(&mut source, &dock);
            assert!(dock.host.borrow().mounts[&source_window.window_id()].waiting_for_update);

            if move_pane {
                assert!(!source.update(|window, _| window.is_focus_handle_rendered(&focus)));
            }
            if move_pane || click {
                press(&mut source, old_content, managed);
                release_drag(&mut source, old_content, managed);
                assert_eq!(
                    editors[0].read_with(&source, |editor, _| editor.clicks),
                    usize::from(click)
                );
            }
            source.simulate_keystrokes("b");
            assert_eq!(
                editors[0].read_with(&source, |editor, _| editor.text.clone()),
                if move_pane || blur { "a" } else { "ab" },
            );

            cx.update(|_| assert!(pending.complete(PlatformWindowPresentOutcome::Submitted)));
            cx.set_window_frame_delivery_mode(
                sibling_window,
                PlatformFrameDeliveryMode::Synchronous,
            );
            frames(&mut sibling, &dock);
            frames(&mut source, &dock);
            assert!(
                source.update(|window, cx| {
                    if blur {
                        window.focused(cx).is_none()
                    } else if move_pane || click {
                        focus.is_focused(window)
                    } else {
                        destination.is_focused(window)
                    }
                }),
                "managed={managed}, moved={move_pane}, blur={blur}, click={click}"
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
            region.presented_panes.as_ref().unwrap().panes[&FIRST]
                .geometry
                .clone(),
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
        assert!(
            mount
                .input_frames
                .iter()
                .all(|frame| frame.surface != SURFACE)
        );
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
        let geometry = &frame.panes[&FIRST].geometry;
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
