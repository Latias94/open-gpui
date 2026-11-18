//! Independent key events may arrive before another retained packet is painted.

use super::*;
use open_gpui::Keystroke;

fn focus_tab(item: ItemId, visual: &mut VisualTestContext) {
    let label = format!("Pane {}", item.get());
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (target, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Tab && node.label() == Some(label.as_str()))
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Focus,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *target,
            data: None,
        })
    );
}

#[open_gpui::test]
fn tab_navigation_round_trip_and_activation_do_not_require_an_intermediate_paint(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.simulate_keystrokes("right left");
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    frames(&mut visual, &dock);
    visual.simulate_keystrokes("end enter");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    let version = dock.host.borrow().session.version();
    frames(&mut visual, &dock);
    assert_eq!(dock.host.borrow().session.version(), version);
}

#[open_gpui::test]
fn repeated_navigation_and_document_cycles_reach_the_third_tab_between_submission_frames(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let third = ItemId::new(63);
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let view = cx.new(|_| Pane {
            focus: focus.clone(),
        });
        dock.open_pane(
            DockPane::new(third, "Pane 63", view).with_focus_handle(focus),
            dockspace::model::DockPlacement::Center(dockspace::model::DockAnchor::Item(FIRST)),
            cx,
        )
        .unwrap();
        dock.update(
            |frame| {
                frame.select_item_current(FIRST)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.simulate_keystrokes("right right");
    assert!(dock.with_view(|view| view.item(third).unwrap().is_active()));
    frames(&mut visual, &dock);
    visual.simulate_keystrokes("ctrl-shift-tab ctrl-shift-tab");
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Tab, "Pane 61");
}

#[open_gpui::test]
fn a_new_application_blur_supersedes_keyboard_continuation(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, false);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.simulate_keystrokes("right");
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    visual.update(|window, cx| window.blur(cx));
    visual.simulate_keystrokes("left");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| window.focused(cx).is_none()));
}

#[open_gpui::test]
fn managed_header_navigation_keeps_its_original_window_and_content_focus(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, handles) = fixture_with_mode(cx, false, true);
    let window = cx
        .update(|cx| dock.open_native_root(SURFACE, true, Default::default(), cx))
        .unwrap();
    let mut visual = active_visual(window, cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let binding = dock.native_binding_for_window(window.window_id()).unwrap();
    focus_tab(FIRST, &mut visual);
    visual.simulate_keystrokes("right left");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_eq!(
        dock.native_binding_for_window(window.window_id()),
        Some(binding)
    );
    visual.simulate_keystrokes("end enter");
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn focused_splitter_restores_preferred_sizing_only_on_explicit_activation(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture(cx, true);
    cx.update(|cx| {
        dock.update_pane_metadata(
            FIRST,
            "Pane 61",
            dockspace::geometry::PaneSizing::builder()
                .min_width(120.0)
                .preferred_width(250.0)
                .build()
                .unwrap(),
            cx,
        )
    })
    .unwrap();
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let before = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (target, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Splitter)
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Focus,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *target,
            data: None,
        })
    );
    visual.simulate_keystrokes("enter");
    frames(&mut visual, &dock);
    let preferred = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(preferred.origin.x < before.origin.x);
    assert!((f32::from(preferred.center().x) - 250.0).abs() < 4.0);
    visual.simulate_keystrokes("right");
    frames(&mut visual, &dock);
    let manually_resized = receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter);
    assert!(manually_resized.origin.x > preferred.origin.x);
    frames(&mut visual, &dock);
    assert_eq!(
        receiver_bounds(&dock, &mut visual, DockspaceReceiverRole::Splitter),
        manually_resized
    );
}

type PaneKeys = Rc<RefCell<Vec<(ItemId, String)>>>;

struct KeyPane {
    item: ItemId,
    focus: FocusHandle,
    keys: PaneKeys,
}

impl Render for KeyPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let item = self.item;
        let keys = self.keys.clone();
        div()
            .id("keyboard-pane")
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(move |event, _, _| {
                keys.borrow_mut().push((item, event.keystroke.key.clone()));
            })
            .child("Content")
    }
}

fn tab_fixture(
    cx: &mut TestAppContext,
    items: &[ItemId],
) -> (DockSurface, Vec<FocusHandle>, PaneKeys) {
    cx.update(|cx| {
        let keys = PaneKeys::default();
        let handles = items.iter().map(|_| cx.focus_handle()).collect::<Vec<_>>();
        let panes = items
            .iter()
            .zip(&handles)
            .map(|(&item, focus)| {
                let view = cx.new(|_| KeyPane {
                    item,
                    focus: focus.clone(),
                    keys: keys.clone(),
                });
                DockPane::new(item, format!("Pane {}", item.get()), view)
                    .with_focus_handle(focus.clone())
            })
            .collect::<Vec<_>>();
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(71), DockspaceNode::tabs(items.iter().copied())),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, handles, keys)
    })
}

fn core_tab_list_fixture(
    cx: &mut TestAppContext,
) -> (
    DockSurface,
    Vec<FocusHandle>,
    PaneKeys,
    VisualTestContext,
    Rc<RefCell<Option<Rc<InteractionState>>>>,
) {
    cx.defer_next_window_frame_requests();
    let (dock, handles, keys) = tab_fixture(cx, &[FIRST, SECOND, ItemId::new(63)]);
    let interaction = Rc::new(RefCell::new(None));
    let window = cx.open_window(size(px(160.0), px(600.0)), |_, _| FocusStateRoot {
        dock: dock.clone(),
        interaction: interaction.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
    (dock, handles, keys, visual, interaction)
}

#[open_gpui::test]
fn core_tab_list_down_down_enter_preserves_the_same_update_prefix(cx: &mut TestAppContext) {
    let (dock, handles, keys, mut visual, _) = core_tab_list_fixture(cx);
    visual.update(|window, cx| {
        for key in ["down", "down", "enter"] {
            assert!(
                window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx),
                "the original menu must own {key}"
            );
        }
        assert!(
            !window.dispatch_keystroke(Keystroke::parse("up").unwrap(), cx),
            "confirmation ends the menu prefix before a replacement is painted"
        );
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(ItemId::new(63)).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[2].contains_focused(window, cx)));
    visual.update(|window, cx| {
        assert!(!window.dispatch_keystroke(Keystroke::parse("up").unwrap(), cx));
    });
    assert_eq!(
        keys.borrow().last(),
        Some(&(ItemId::new(63), "up".to_owned()))
    );
}

#[open_gpui::test]
fn core_tab_list_up_boundary_clamps_before_reversing(cx: &mut TestAppContext) {
    let (dock, handles, _, mut visual, _) = core_tab_list_fixture(cx);
    visual.update(|window, cx| {
        for key in ["up", "down", "enter"] {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        }
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
}

#[open_gpui::test]
fn core_tab_list_end_boundary_clamps_before_reversing(cx: &mut TestAppContext) {
    let (dock, handles, _, mut visual, _) = core_tab_list_fixture(cx);
    visual.update(|window, cx| {
        for key in ["end", "down", "up", "enter"] {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        }
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
}

#[open_gpui::test]
fn core_tab_list_home_uses_the_destination_of_the_preceding_key(cx: &mut TestAppContext) {
    let (dock, handles, _, mut visual, _) = core_tab_list_fixture(cx);
    visual.update(|window, cx| {
        for key in ["end", "home", "down", "enter"] {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        }
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    assert!(visual.update(|window, cx| handles[1].contains_focused(window, cx)));
}

#[open_gpui::test]
fn core_tab_list_prefix_continues_through_an_unpresented_frame(cx: &mut TestAppContext) {
    let (dock, handles, _, mut visual, interaction) = core_tab_list_fixture(cx);
    let interaction = interaction.borrow().as_ref().unwrap().clone();
    let window = visual.update(|window, _| window.window_handle());
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx));
    });
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    visual.update(|window, cx| {
        {
            let host = dock.host.borrow();
            let owner = window.window_handle().window_id();
            let serial = host.mounts[&owner].serial;
            assert!(
                !input::is_presented(&host, owner, SURFACE, serial),
                "an encoded replacement must not supply keyboard authority"
            );
        }
        for key in ["down", "enter"] {
            assert!(
                window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx),
                "the original menu prefix must retain {key} through an inert frame"
            );
        }
        assert!(!window.dispatch_keystroke(Keystroke::parse("up").unwrap(), cx));
    });
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    let mut trace = Vec::new();
    for frame in 0..5 {
        if frame < 4 {
            assert!(visual.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
        } else {
            // The selected pane first becomes interactive in the fourth replay.
            // Normal accepted-publication focus runs after authority is sealed,
            // so its claim commits in the next generation. That frame must be
            // scheduled by GPUI itself, rather than forced by this test.
            assert!(
                visual.simulate_scheduled_frame(),
                "pane focus must schedule its next-generation commit:\n{}",
                trace.join("\n")
            );
        }
        assert!(dock.error().is_none(), "{:?}", dock.error());
        trace.push(visual.update(|window, cx| {
            let host = dock.host.borrow();
            let mount = &host.mounts[&window.window_handle().window_id()];
            let region = &mount.regions[&SURFACE];
            let attempts = interaction.pane_focus.borrow();
            let attempt = attempts
                .get(&SURFACE)
                .map(|attempt| (attempt.request, attempt.applied, attempt.observation));
            format!(
                "frame={frame} serial={} presented={:?} version={:?} selection63={} \
                 focused={:?} committed={:?} claim={} rendered63={} focused63={} \
                 request={:?} attempt={attempt:?} retained={}",
                mount.serial,
                region.presented,
                host.session.version(),
                host.session
                    .view()
                    .item(ItemId::new(63))
                    .unwrap()
                    .is_active(),
                window.focused(cx),
                window.committed_focus(cx),
                window.focus_claim_revision(),
                window.is_focus_handle_rendered(&handles[2]),
                handles[2].contains_focused(window, cx),
                host.session.pane_focus_request(SURFACE),
                interaction.keyboard.borrow().is_some(),
            )
        }));
        if visual.update(|window, cx| handles[2].contains_focused(window, cx)) {
            break;
        }
    }
    assert!(dock.with_view(|view| view.item(ItemId::new(63)).unwrap().is_active()));
    assert!(
        visual.update(|window, cx| handles[2].contains_focused(window, cx)),
        "deferred menu confirmation must focus the selected pane:\n{}",
        trace.join("\n")
    );
}

#[open_gpui::test]
fn core_tab_list_cancel_ends_the_same_update_prefix(cx: &mut TestAppContext) {
    let (dock, _, _, mut visual, interaction) = core_tab_list_fixture(cx);
    let interaction = interaction.borrow().as_ref().unwrap().clone();
    visual.update(|window, cx| {
        let opener = interaction
            .tab_list_focus
            .borrow()
            .as_ref()
            .unwrap()
            .opener
            .clone();
        for key in ["down", "escape"] {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        }
        assert!(
            opener.is_focused(window),
            "cancellation hands the suffix back to the original opener immediately"
        );
        let version = dock.host.borrow().session.version();
        assert!(
            !window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx),
            "the opener owns the suffix after menu cancellation"
        );
        assert_eq!(dock.host.borrow().session.version(), version);
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_accessibility_focus(&visual, Role::Button, "Show all tabs");
}

#[open_gpui::test]
fn core_tab_list_cancel_then_opener_down_preserves_the_reopen_order(cx: &mut TestAppContext) {
    let (dock, _, _, mut visual, interaction) = core_tab_list_fixture(cx);
    let interaction = interaction.borrow().as_ref().unwrap().clone();
    let original_row = interaction.tab_list_focus.borrow().as_ref().unwrap().row;
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx));
    });
    // The first update committed its focus move, but its replacement menu has
    // not been presented. Dismissal must precede the opener's later command.
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx));
        assert!(
            window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx),
            "the opener's Down must follow dismissal"
        );
        assert!(
            !window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx),
            "the terminal barrier only retains this opener's semantic input"
        );
    });
    for _ in 0..3 {
        frames(&mut visual, &dock);
    }
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
    let reopened_row = interaction.tab_list_focus.borrow().as_ref().unwrap().row;
    assert_ne!(
        reopened_row, original_row,
        "the opener must start a new core menu session after dismissal"
    );
}

#[open_gpui::test]
fn core_tab_list_pending_prefix_cannot_override_an_application_focus_claim(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND, ItemId::new(63)]);
    let focus = cx.update(|cx| cx.focus_handle());
    let keys = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(160.0), px(600.0)), |_, _| ExternalFocusRoot {
        dock: dock.clone(),
        focus: focus.clone(),
        keys: keys.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let button = tab_list_button_bounds(&dock, window.window_id());
    visual.simulate_click(button.center(), Modifiers::none());
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::MenuItem, "Pane 61");
    let claim = visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx));
        assert!(window.is_focus_handle_rendered(&focus));
        window.focus(&focus, cx);
        let claim = window.focus_claim_revision();
        assert!(!window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        claim
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    visual.update(|window, cx| {
        assert!(focus.contains_focused(window, cx));
        assert_eq!(window.focus_claim_revision(), claim);
        assert!(!window.dispatch_keystroke(Keystroke::parse("up").unwrap(), cx));
    });
    assert_eq!(keys.borrow().last().map(String::as_str), Some("up"));
}

#[open_gpui::test]
fn core_tab_list_pending_prefix_cannot_acquire_a_replacement_epoch(cx: &mut TestAppContext) {
    let (dock, _, _, mut visual, _) = core_tab_list_fixture(cx);
    let layout = dock.with_view(|view| view.to_layout());
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx));
    });
    let previous = dock.host.borrow().session.version();
    let reset = dock
        .host
        .borrow()
        .session
        .prepare_layout_reset(layout)
        .unwrap();
    let replacement = visual.update(|window, cx| {
        dock.update(
            |frame| {
                frame.submit_layout_reset(&reset)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let replacement = dock.host.borrow().session.version();
        assert!(!previous.is_same_epoch(replacement));
        assert!(!window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        replacement
    });
    frames(&mut visual, &dock);
    assert_eq!(dock.host.borrow().session.version(), replacement);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    let tree = visual.latest_accessibility_tree_update().unwrap();
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(id, node)| *id == tree.focus && node.role() == Role::MenuItem),
        "a replacement epoch cannot restore focus from the old menu prefix"
    );
}

struct ExternalFocusRoot {
    dock: DockSurface,
    focus: FocusHandle,
    keys: Rc<RefCell<Vec<String>>>,
}

impl Render for ExternalFocusRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let keys = self.keys.clone();
        div().size_full().child(self.dock.element(SURFACE)).child(
            div()
                .id("application-keyboard-target")
                .absolute()
                .bottom_0()
                .right_0()
                .size(px(1.0))
                .track_focus(&self.focus)
                .on_key_down(move |event, _, _| {
                    keys.borrow_mut().push(event.keystroke.key.clone());
                }),
        )
    }
}

struct FocusStateRoot {
    dock: DockSurface,
    interaction: Rc<RefCell<Option<Rc<InteractionState>>>>,
}

impl Render for FocusStateRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let dock = self.dock.clone();
        let interaction = self.interaction.clone();
        div().size_full().child(self.dock.element(SURFACE)).child(
            open_gpui::canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    let id = dock.host.borrow().mounts[&window.window_handle().window_id()]
                        .element
                        .clone()
                        .unwrap();
                    window.with_element_state::<Rc<InteractionState>, _>(&id, |state, _| {
                        let state = state.unwrap();
                        *interaction.borrow_mut() = Some(state.clone());
                        ((), state)
                    });
                },
            )
            .absolute()
            .size(px(1.0)),
        )
    }
}

#[open_gpui::test]
fn document_cycles_start_at_selection_while_header_arrows_start_at_focus(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let third = ItemId::new(63);
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND, third]);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    for (key, expected) in [
        ("ctrl-tab", SECOND),
        ("ctrl-shift-tab", third),
        ("ctrl-pagedown", SECOND),
        ("ctrl-pageup", third),
        ("right", third),
    ] {
        cx.update(|cx| {
            dock.update(
                |frame| {
                    frame.select_item_current(FIRST)?;
                    Ok(())
                },
                cx,
            )
        })
        .unwrap();
        frames(&mut visual, &dock);
        focus_tab(SECOND, &mut visual);
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
        visual.update(|window, cx| {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        });
        assert!(
            dock.with_view(|view| view.item(expected).unwrap().is_active()),
            "{key}"
        );
        frames(&mut visual, &dock);
        assert_accessibility_focus(&visual, Role::Tab, &format!("Pane {}", expected.get()));
    }
}

#[open_gpui::test]
fn activation_yields_the_same_update_key_suffix_to_rendered_content(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, handles, keys) = tab_fixture(cx, &[FIRST, SECOND]);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        let request = dock
            .host
            .borrow()
            .session
            .pane_focus_request(SURFACE)
            .unwrap();
        assert_eq!(request.item(), Some(FIRST));
        assert!(handles[0].contains_focused(window, cx));
        assert!(!window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx));
        assert_eq!(
            dock.host.borrow().session.pane_focus_request(SURFACE),
            Some(request)
        );
    });
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    frames(&mut visual, &dock);
    assert!(visual.update(|window, cx| handles[0].contains_focused(window, cx)));
    visual.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("left").unwrap(), cx);
    });
    assert_eq!(keys.borrow().last(), Some(&(FIRST, "left".to_owned())));
}

#[open_gpui::test]
fn clipped_end_target_activates_before_left_yields_to_content(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let items = (61..101).map(ItemId::new).collect::<Vec<_>>();
    let last = *items.last().unwrap();
    let (dock, handles, keys) = tab_fixture(cx, &items);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        {
            let host = dock.host.borrow();
            let packet = host.mounts[&window.window_handle().window_id()].regions[&SURFACE].packet.as_ref().unwrap();
            assert!(!packet.draws.iter().any(|draw| matches!(draw,
                Draw::Control(control) if matches!(control.kind, ControlKind::Tab(key) if key == last.into())
            )), "the destination must have no control in the original packet");
            assert!(!window.is_focus_handle_rendered(handles.last().unwrap()));
        }
        assert!(window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx));
        assert!(window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        let request = dock.host.borrow().session.pane_focus_request(SURFACE).unwrap();
        assert_eq!(request.item(), Some(last));
        assert!(!window.dispatch_keystroke(Keystroke::parse("left").unwrap(), cx));
        assert_eq!(dock.host.borrow().session.pane_focus_request(SURFACE), Some(request));
    });
    assert!(
        keys.borrow().is_empty(),
        "the clipped content has not been rendered yet"
    );
    assert!(dock.with_view(|view| view.item(last).unwrap().is_active()));
    frames(&mut visual, &dock);
    assert!(visual.update(|window, cx| handles.last().unwrap().contains_focused(window, cx)));
}

#[open_gpui::test]
fn clipped_navigation_in_an_earlier_update_still_allows_content_activation(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let items = (61..101).map(ItemId::new).collect::<Vec<_>>();
    let last = *items.last().unwrap();
    let (dock, handles, _) = tab_fixture(cx, &items);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx));
    });
    assert!(dock.with_view(|view| view.item(last).unwrap().is_active()));
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        assert!(
            dock.host
                .borrow()
                .session
                .pane_focus_request(SURFACE)
                .is_none()
        );
        assert!(!window.dispatch_keystroke(Keystroke::parse("left").unwrap(), cx));
    });
    for _ in 0..8 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
        if visual.update(|window, cx| handles.last().unwrap().contains_focused(window, cx)) {
            break;
        }
    }
    assert!(visual.update(|window, cx| handles.last().unwrap().contains_focused(window, cx)));
}

#[open_gpui::test]
fn an_application_blur_cancels_clipped_activation_waiting_for_presentation(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let items = (61..101).map(ItemId::new).collect::<Vec<_>>();
    let last = *items.last().unwrap();
    let (dock, _, _) = tab_fixture(cx, &items);
    let window = cx.open_window(size(px(640.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.simulate_keystrokes("end");
    visual.simulate_keystrokes("enter");
    visual.update(|window, cx| window.blur(cx));
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.item(last).unwrap().is_active()));
    assert!(visual.update(|window, cx| window.focused(cx).is_none()));
    assert!(
        dock.host
            .borrow()
            .session
            .pane_focus_request(SURFACE)
            .is_none()
    );
}

#[open_gpui::test]
fn one_update_navigation_round_trip_and_cycles_preserve_event_order(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let third = ItemId::new(63);
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND, third]);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    for (keys, expected) in [
        (["right", "left"], FIRST),
        (["right", "right"], third),
        (["ctrl-shift-tab", "ctrl-shift-tab"], FIRST),
    ] {
        visual.update(|window, cx| {
            for key in keys {
                assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
            }
        });
        assert!(dock.with_view(|view| view.item(expected).unwrap().is_active()));
    }
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Tab, "Pane 61");
}

#[open_gpui::test]
fn a_later_content_focus_claim_cancels_the_unsubmitted_key_prefix(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, handles, _) = tab_fixture(cx, &[FIRST, SECOND]);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    let claim = visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx));
        window.focus(&handles[0], cx);
        window.focus_claim_revision()
    });
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    frames(&mut visual, &dock);
    visual.update(|window, cx| {
        assert!(handles[0].contains_focused(window, cx));
        assert_eq!(window.focus_claim_revision(), claim);
    });
}

#[open_gpui::test]
fn same_epoch_reordering_rejects_the_original_keyboard_scope(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let third = ItemId::new(63);
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND, third]);
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx);
    });
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    let before = dock.host.borrow().session.version();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.dock_item_current(third, dockspace::model::DockPlacement::Before(FIRST))?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    let reordered = dock.host.borrow().session.version();
    assert!(before.is_same_epoch(reordered));
    assert_ne!(before, reordered);
    visual.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx);
    });
    assert_eq!(dock.host.borrow().session.version(), reordered);
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
}

#[open_gpui::test]
fn layout_reset_rejects_a_key_continuation_before_another_frame(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND]);
    let layout = dock.with_view(|view| view.to_layout());
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx);
    });
    assert!(dock.with_view(|view| view.item(SECOND).unwrap().is_active()));
    let previous = dock.host.borrow().session.version();
    let reset = dock
        .host
        .borrow()
        .session
        .prepare_layout_reset(layout)
        .unwrap();
    let replacement = visual.update(|window, cx| {
        dock.update(
            |frame| {
                frame.submit_layout_reset(&reset)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let replacement = dock.host.borrow().session.version();
        assert!(!previous.is_same_epoch(replacement));
        window.dispatch_keystroke(Keystroke::parse("left").unwrap(), cx);
        replacement
    });
    assert_eq!(dock.host.borrow().session.version(), replacement);
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
    frames(&mut visual, &dock);
    assert!(dock.error().is_none());
}

#[open_gpui::test]
fn layout_reset_cannot_restore_pending_header_focus_or_refresh_its_witness(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (dock, _, _) = tab_fixture(cx, &[FIRST, SECOND]);
    let layout = dock.with_view(|view| view.to_layout());
    let interaction = Rc::new(RefCell::new(None));
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| FocusStateRoot {
        dock: dock.clone(),
        interaction: interaction.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let original_ids = visual.update(|window, _| {
        dock.host.borrow().mounts[&window.window_handle().window_id()].regions[&SURFACE]
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Control(control) if matches!(control.kind, ControlKind::Tab(_)) => {
                    Some(control.id)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    focus_tab(FIRST, &mut visual);
    visual.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx);
    });
    let interaction = interaction.borrow().as_ref().unwrap().clone();
    assert!(interaction.keyboard.borrow().is_some());
    assert!(interaction.pending_tab_focus.borrow().is_some());
    let previous = dock.host.borrow().session.version();
    let reset = dock
        .host
        .borrow()
        .session
        .prepare_layout_reset(layout)
        .unwrap();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.submit_layout_reset(&reset)?;
                Ok(())
            },
            cx,
        )
    })
    .unwrap();
    assert!(!previous.is_same_epoch(dock.host.borrow().session.version()));
    frames(&mut visual, &dock);
    visual.update(|window, _| {
        let host = dock.host.borrow();
        let replacement_ids = host.mounts[&window.window_handle().window_id()].regions[&SURFACE]
            .packet
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Control(control) if matches!(control.kind, ControlKind::Tab(_)) => {
                    Some(control.id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            original_ids, replacement_ids,
            "stable IDs must not provide replacement authority"
        );
        assert!(
            interaction.keyboard.borrow().is_none(),
            "an old continuation cannot acquire the replacement packet's witness"
        );
        assert!(
            interaction.pending_tab_focus.borrow().is_none(),
            "an old header focus request cannot cross the reset epoch"
        );
    });
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().is_active()));
}
