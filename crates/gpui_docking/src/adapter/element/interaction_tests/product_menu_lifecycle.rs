use open_gpui::{KeyUpEvent, Keystroke, PlatformInput};

use crate::adapter::DockRegion;

use super::{
    wheel::{
        RegionsRoot, SingleRegion, overflowing_surfaces_with_mode, tab_list_accessible_action,
    },
    *,
};

fn key_down(window: &mut Window, key: &'static str, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }),
        cx,
    );
}

fn key_up(window: &mut Window, key: &'static str, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::KeyUp(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        }),
        cx,
    );
}

fn key_pair(window: &mut Window, key: &'static str, cx: &mut App) {
    key_down(window, key, cx);
    key_up(window, key, cx);
}

fn opener_action(visual: &VisualTestContext, action: AccessibleAction) {
    menu_button_action(visual, "Layout actions", action);
}

fn menu_button_action(visual: &VisualTestContext, label: &str, action: AccessibleAction) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let target = tree
        .nodes
        .iter()
        .find_map(|(target, node)| {
            (node.role() == Role::Button && node.label() == Some(label)).then_some(*target)
        })
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        })
    );
}

fn open_menu(dock: &DockSurface, visual: &mut VisualTestContext) {
    opener_action(visual, AccessibleAction::Click);
    frames(visual, dock);
    assert!(presentation_menu_is_open(visual));
    assert_accessibility_focus(visual, Role::Button, "Layout actions");
    assert_eq!(
        selected_presentation_menu_row(visual).as_deref(),
        Some("Float Layout")
    );
}

fn assert_floated(dock: &DockSurface) {
    dock.with_view(|view| {
        let contained = view.item(FIRST).unwrap().contained();
        assert!(
            contained.is_some(),
            "the original confirmation must execute Float Layout"
        );
        assert_eq!(view.item(SECOND).unwrap().contained(), contained);
        assert_eq!(view.surface(SURFACE).unwrap().contained_count(), 1);
    });
}

fn scheduled_frames(visual: &mut VisualTestContext) {
    let owner = visual.update(|window, _| window.window_handle());
    for _ in 0..4 {
        if !visual.step_deferred_window_frame_request(owner) && !visual.simulate_scheduled_frame() {
            break;
        }
    }
}

fn shutdown(dock: &DockSurface, cx: &TestAppContext) {
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}

struct WakeRoot {
    dock: DockSurface,
    application_focus: FocusHandle,
    wake_after_normal: Rc<Cell<bool>>,
}

impl Render for WakeRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let transport = self.dock.native_transport().unwrap();
        let armed = self.wake_after_normal.clone();
        div()
            .size_full()
            .track_focus(&self.application_focus)
            .child(self.dock.element(SURFACE))
            .on_children_prepainted(move |_, window, _| {
                let transport = transport.clone();
                let armed = armed.clone();
                window.record_prepaint_focus_stable_commit(move |_, _, cx| {
                    if armed.replace(false) {
                        transport.wake(cx);
                    }
                });
            })
    }
}

fn managed_menu(
    cx: &mut TestAppContext,
) -> (DockSurface, FocusHandle, VisualTestContext, Rc<Cell<bool>>) {
    cx.defer_next_window_frame_requests();
    let (dock, _) = fixture_with_mode(cx, false, true);
    let application_focus = cx.update(|cx| cx.focus_handle());
    let wake_after_normal = Rc::new(Cell::new(false));
    let window = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
        dock.bind_native_root(SURFACE, true, window, cx).unwrap();
        WakeRoot {
            dock: dock.clone(),
            application_focus: application_focus.clone(),
            wake_after_normal: wake_after_normal.clone(),
        }
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    open_menu(&dock, &mut visual);
    (dock, application_focus, visual, wake_after_normal)
}

#[open_gpui::test]
fn product_menu_real_press_and_release_open_only_once(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (dock, _, mut visual, _) = presentation_menu_with_content_focus(cx);
        opener_action(&visual, AccessibleAction::Focus);
        frames(&mut visual, &dock);
        visual.update(|window, cx| key_pair(window, key, cx));
        frames(&mut visual, &dock);
        assert!(presentation_menu_is_open(&visual), "{key} must open once");
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
    }
}

#[open_gpui::test]
fn product_menu_focus_return_accepts_a_fresh_keyboard_request(cx: &mut TestAppContext) {
    for leave_opener in [false, true] {
        let (dock, content_focus, mut visual, _) = presentation_menu_with_content_focus(cx);
        open_menu(&dock, &mut visual);
        if leave_opener {
            visual.update(|window, cx| window.focus(&content_focus, cx));
            frames(&mut visual, &dock);
        }
        opener_action(&visual, AccessibleAction::Focus);
        frames(&mut visual, &dock);
        assert_accessibility_focus(&visual, Role::Button, "Layout actions");

        visual.update(|window, cx| key_pair(window, "down", cx));
        frames(&mut visual, &dock);
        assert!(presentation_menu_is_open(&visual));
        visual.update(|window, cx| key_pair(window, "end", cx));
        frames(&mut visual, &dock);
        assert_eq!(
            selected_presentation_menu_row(&visual).as_deref(),
            Some("Close Layout"),
            "a new key must work after opener focus is explicitly claimed again"
        );
        visual.update(|window, cx| key_pair(window, "escape", cx));
        frames(&mut visual, &dock);
        assert!(!presentation_menu_is_open(&visual));
        assert!(dock.pending_content_close_plans().unwrap().is_empty());
    }
}

#[open_gpui::test]
fn product_menu_deactivation_cancels_unpresented_group_and_layout_commands(
    cx: &mut TestAppContext,
) {
    for managed in [false, true] {
        for (label, keys) in [
            ("Layout actions", &["down", "enter"][..]),
            ("Tab group actions", &["down", "end", "enter"][..]),
        ] {
            cx.defer_next_window_frame_requests();
            let other_focus = cx.update(|cx| cx.focus_handle());
            let other = cx.open_window(size(px(320.0), px(240.0)), |_, _| Pane {
                focus: other_focus.clone(),
            });
            let (dock, _) = fixture_with_mode(cx, false, managed);
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
                if managed {
                    dock.bind_native_root(SURFACE, true, window, cx).unwrap();
                }
                Root { dock: dock.clone() }
            });
            let mut visual = active_visual(window.into(), cx);
            assert!(visual.activate_accessibility());
            frames(&mut visual, &dock);
            menu_button_action(&visual, label, AccessibleAction::Focus);
            frames(&mut visual, &dock);
            visual
                .set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
            visual.update(|window, cx| {
                for key in keys {
                    assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
                }
            });

            let mut foreground = active_visual(other.into(), cx);
            assert!(foreground.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            foreground.update(|window, cx| window.focus(&other_focus, cx));
            assert!(!visual.update(|window, _| window.is_window_active()));
            visual
                .set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
            frames(&mut visual, &dock);
            frames(&mut visual, &dock);
            assert!(foreground.update(|window, cx| other_focus.contains_focused(window, cx)));

            // Neither confirmation may appear in core while the other window
            // is active or when the original window becomes active again.
            let assert_unchanged = || {
                assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
                assert!(dock.pending_content_close_plans().unwrap().is_empty());
            };
            assert_unchanged();
            let mut visual = active_visual(window.into(), cx);
            frames(&mut visual, &dock);
            assert_unchanged();
            if managed {
                shutdown(&dock, cx);
            }
        }
    }
}

#[open_gpui::test]
fn product_menu_deactivation_revokes_accessible_commands(cx: &mut TestAppContext) {
    for managed in [false, true] {
        cx.defer_next_window_frame_requests();
        let other_focus = cx.update(|cx| cx.focus_handle());
        let other = cx.open_window(size(px(320.0), px(240.0)), |_, _| Pane {
            focus: other_focus.clone(),
        });
        let (dock, _) = fixture_with_mode(cx, false, managed);
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(800.0), px(600.0)), |window, cx| {
            if managed {
                dock.bind_native_root(SURFACE, true, window, cx).unwrap();
            }
            Root { dock: dock.clone() }
        });
        let mut visual = active_visual(window.into(), cx);
        assert!(visual.activate_accessibility());
        frames(&mut visual, &dock);
        for (label, command) in [
            ("Layout actions", "Float Layout"),
            ("Tab group actions", "Close Tab Group"),
        ] {
            menu_button_action(&visual, label, AccessibleAction::Click);
            frames(&mut visual, &dock);
            let target = visual
                .latest_accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .find_map(|(id, node)| {
                    (node.role() == Role::MenuItem && node.label() == Some(command)).then_some(*id)
                })
                .unwrap();
            let _foreground = active_visual(other.into(), cx);
            assert!(!visual.update(|window, _| window.is_window_active()));
            // Deliver the old accessibility action before the inactive window redraws.
            visual.dispatch_accessibility_action(accesskit::ActionRequest {
                action: AccessibleAction::Click,
                target_tree: accesskit::TreeId::ROOT,
                target_node: target,
                data: None,
            });
            frames(&mut visual, &dock);
            assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
            assert!(dock.pending_content_close_plans().unwrap().is_empty());
            assert!(!visual
                .latest_accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::MenuItem && node.label() == Some(command)));
            visual = active_visual(window.into(), cx);
            frames(&mut visual, &dock);
        }
        open_menu(&dock, &mut visual);
        if managed {
            shutdown(&dock, cx);
        }
    }
}

#[open_gpui::test]
fn product_menu_real_key_up_cannot_cancel_confirmation_waiting_for_native_fifo(
    cx: &mut TestAppContext,
) {
    confirm_with_real_key_up(cx, "enter");
}

#[open_gpui::test]
fn product_menu_space_key_up_preserves_confirmation_waiting_for_native_fifo(
    cx: &mut TestAppContext,
) {
    confirm_with_real_key_up(cx, "space");
}

fn confirm_with_real_key_up(cx: &mut TestAppContext, key: &'static str) {
    // Closing the last managed window can terminate the application on Windows.
    // Each key needs its own app lifetime rather than reopening a stopped app.
    let (dock, _, mut visual, _) = managed_menu(cx);
    let transport = dock.native_transport().unwrap();
    visual.update(|window, cx| {
        transport.wake(cx);
        assert_eq!(transport.inspection().front_record, Some("wake"));
        key_down(window, key, cx);
        assert_eq!(transport.inspection().front_record, Some("surface-action"));
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
        key_up(window, key, cx);
    });
    visual.run_until_parked();
    assert!(transport.is_empty());
    assert_floated(&dock);
    scheduled_frames(&mut visual);
    assert!(!presentation_menu_is_open(&visual));
    shutdown(&dock, cx);
}

#[open_gpui::test]
fn product_menu_application_focus_cancels_confirmation_even_after_focus_returns(
    cx: &mut TestAppContext,
) {
    let (dock, application_focus, mut visual, _) = managed_menu(cx);
    visual.update(|window, cx| key_pair(window, "escape", cx));
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    let owner = visual.update(|window, _| window.window_handle());
    visual.set_window_present_outcome(owner, PlatformWindowPresentOutcome::Deferred);
    let claim = visual.update(|window, cx| {
        key_pair(window, "down", cx);
        key_down(window, "enter", cx);
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
        let host = dock.host.borrow();
        let mount = &host.mounts[&owner.window_id()];
        assert!(!input::is_presented(
            &host,
            owner.window_id(),
            SURFACE,
            mount.serial
        ));
        drop(host);
        assert!(window.is_focus_handle_rendered(&application_focus));
        window.focus(&application_focus, cx);
        let claim = window.focus_claim_revision();
        key_up(window, "enter", cx);
        claim
    });
    frames(&mut visual, &dock);
    visual.set_window_present_outcome(owner, PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    visual.update(|window, _| {
        assert!(application_focus.is_focused(window));
        assert_eq!(window.focus_claim_revision(), claim);
    });
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));

    opener_action(&visual, AccessibleAction::Focus);
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
    visual.update(|window, cx| key_pair(window, "end", cx));
    frames(&mut visual, &dock);
    assert_eq!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Close Layout"),
        "the new prefix must discard the old unpresented confirmation"
    );
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
    assert!(dock.pending_content_close_plans().unwrap().is_empty());
    shutdown(&dock, cx);
}

#[open_gpui::test]
fn product_menu_confirmation_survives_a_late_wake_without_core_repaint(cx: &mut TestAppContext) {
    let (dock, _, mut visual, wake_after_normal) = managed_menu(cx);
    let transport = dock.native_transport().unwrap();
    visual.update(|window, cx| {
        transport.wake(cx);
        assert_eq!(transport.inspection().front_record, Some("wake"));
        key_down(window, "enter", cx);
        assert_eq!(transport.inspection().front_record, Some("surface-action"));
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
    });
    visual.run_until_parked();
    assert!(transport.is_empty());

    wake_after_normal.set(true);
    let owner = visual.update(|window, _| window.window_handle());
    assert!(
        visual.step_deferred_window_frame_request(owner) || visual.simulate_scheduled_frame(),
        "Enter must schedule its own replay"
    );
    visual.run_until_parked();
    assert!(
        !wake_after_normal.get(),
        "the FocusStable wake must be delivered"
    );
    assert!(transport.is_empty());
    scheduled_frames(&mut visual);
    assert_floated(&dock);
    shutdown(&dock, cx);
}

#[open_gpui::test]
fn releasing_the_list_opener_after_a_product_menu_takes_keyboard_ownership(
    cx: &mut TestAppContext,
) {
    #[derive(Clone, Copy, Debug)]
    enum Release {
        Opener,
        OpenerBeforePresentation,
        Elsewhere,
        OpenerAfterResize,
        OpenerAfterResizeThen(&'static str),
    }

    for (product, release) in [
        ("Layout actions", Release::Opener),
        ("Layout actions", Release::OpenerBeforePresentation),
        ("Layout actions", Release::Elsewhere),
        ("Layout actions", Release::OpenerAfterResize),
        ("Layout actions", Release::OpenerAfterResizeThen("enter")),
        ("Layout actions", Release::OpenerAfterResizeThen("escape")),
        ("Tab group actions", Release::Opener),
    ] {
        let (dock, _) = overflowing_surfaces_with_mode(&[SURFACE], true, cx);
        cx.defer_next_window_frame_requests();
        let window = cx.open_window(size(px(620.0), px(600.0)), |window, cx| {
            dock.bind_native_root(SURFACE, true, window, cx).unwrap();
            Root { dock: dock.clone() }
        });
        let mut visual = active_visual(window.into(), cx);
        assert!(visual.activate_accessibility());
        frames(&mut visual, &dock);
        let opener = tab_list_button_bounds(&dock, window.window_id()).center();
        visual.simulate_mouse_down(opener, MouseButton::Left, Modifiers::none());
        menu_button_action(&visual, product, AccessibleAction::Click);
        frames(&mut visual, &dock);
        assert!(
            visual
                .latest_accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Menu && node.label() == Some(product)),
            "{product} must be interactive before {release:?}"
        );
        let rejected_release = matches!(
            release,
            Release::Elsewhere | Release::OpenerAfterResize | Release::OpenerAfterResizeThen(_)
        );
        if rejected_release {
            visual.simulate_keystrokes("end");
            frames(&mut visual, &dock);
            assert_eq!(
                selected_presentation_menu_row(&visual).as_deref(),
                Some("Close Layout")
            );
        }
        let defer_list = matches!(release, Release::OpenerBeforePresentation);
        if defer_list {
            visual.set_window_frame_delivery_mode(
                window.into(),
                open_gpui::PlatformFrameDeliveryMode::Deferred,
            );
        }
        let release_point = match release {
            Release::Opener | Release::OpenerBeforePresentation => opener,
            Release::Elsewhere => point(px(4.0), px(280.0)),
            Release::OpenerAfterResize | Release::OpenerAfterResizeThen(_) => {
                cx.simulate_window_resize(window.into(), size(px(800.0), px(600.0)));
                frames(&mut visual, &dock);
                assert!(presentation_menu_is_open(&visual));
                let moved = tab_list_button_bounds(&dock, window.window_id()).center();
                assert_ne!(moved, opener);
                moved
            }
        };
        visual.simulate_mouse_up(release_point, MouseButton::Left, Modifiers::none());
        if !defer_list && !matches!(release, Release::OpenerAfterResizeThen(_)) {
            frames(&mut visual, &dock);
        }
        visual.simulate_keystrokes(match release {
            Release::OpenerAfterResizeThen(key) => key,
            _ if rejected_release => "enter",
            _ => "end enter",
        });
        if defer_list {
            visual.set_window_frame_delivery_mode(
                window.into(),
                open_gpui::PlatformFrameDeliveryMode::Synchronous,
            );
        }
        frames(&mut visual, &dock);
        let plans = dock.pending_content_close_plans().unwrap();
        if !rejected_release {
            assert!(
                plans.is_empty(),
                "the list's confirmation must never close the layout"
            );
            assert!(dock.with_view(|view| view.item(ItemId::new(40)).unwrap().is_active()));
        } else if matches!(release, Release::OpenerAfterResizeThen("escape")) {
            assert!(plans.is_empty());
            assert!(!presentation_menu_is_open(&visual));
        } else {
            assert_eq!(
                plans.len(),
                1,
                "a release without a matching core popup keeps the product menu active"
            );
            assert_eq!(plans[0].items().len(), 40);
        }
        cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
        cx.run_until_parked();
        assert!(dock.native_is_quiescent());
    }
}

#[open_gpui::test]
fn rejected_list_release_cannot_revive_an_old_product_confirmation(cx: &mut TestAppContext) {
    use open_gpui::PlatformFrameDeliveryMode;

    for product in ["Layout actions", "Tab group actions"] {
        for return_to_product in [false, true] {
            let (dock, _) = overflowing_surfaces_with_mode(&[SURFACE], true, cx);
            let application_focus = cx.update(|cx| cx.focus_handle());
            let keys = Rc::new(RefCell::new(Vec::new()));
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(620.0), px(600.0)), |window, cx| {
                dock.bind_native_root(SURFACE, true, window, cx).unwrap();
                ExternalFocusRoot {
                    dock: dock.clone(),
                    focus: application_focus.clone(),
                    keys: keys.clone(),
                }
            });
            let owner = window.into();
            let mut visual = active_visual(owner, cx);
            assert!(visual.activate_accessibility());
            frames(&mut visual, &dock);
            let opener = tab_list_button_bounds(&dock, window.window_id()).center();
            visual.simulate_mouse_down(opener, MouseButton::Left, Modifiers::none());
            frames(&mut visual, &dock);
            cx.simulate_window_resize(owner, size(px(800.0), px(600.0)));
            frames(&mut visual, &dock);
            let moved_opener = tab_list_button_bounds(&dock, window.window_id()).center();
            assert_ne!(moved_opener, opener);
            menu_button_action(&visual, product, AccessibleAction::Focus);
            frames(&mut visual, &dock);
            assert_accessibility_focus(&visual, Role::Button, product);
            let product_focus = visual.update(|window, cx| window.focused(cx).unwrap());

            visual.set_window_frame_delivery_mode(owner, PlatformFrameDeliveryMode::Deferred);
            visual.update(|window, cx| {
                for key in ["down", "end", "enter"] {
                    key_pair(window, key, cx);
                }
            });
            assert!(visual.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            assert!(visual.pending_window_frame_submission(owner).is_some());
            assert!(dock.pending_content_close_plans().unwrap().is_empty());

            let claim = visual.update(|window, cx| {
                window.focus(&application_focus, cx);
                key_pair(window, "p", cx);
                if return_to_product {
                    window.focus(&product_focus, cx);
                }
                window.focus_claim_revision()
            });
            assert_eq!(&*keys.borrow(), &["p"]);
            visual.simulate_mouse_up(moved_opener, MouseButton::Left, Modifiers::none());
            assert_eq!(
                visual.update(|window, _| window.focus_claim_revision()),
                claim
            );
            visual.set_window_frame_delivery_mode(owner, PlatformFrameDeliveryMode::Synchronous);
            frames(&mut visual, &dock);
            assert!(dock.pending_content_close_plans().unwrap().is_empty());
            assert!(
                visual
                    .latest_accessibility_tree_update()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.role() == Role::Menu && node.label() == Some(product)
                    })
            );
            if !return_to_product {
                visual.update(|window, cx| {
                    assert!(application_focus.is_focused(window));
                    assert_eq!(window.focus_claim_revision(), claim);
                    key_pair(window, "x", cx);
                    key_pair(window, "enter", cx);
                });
                assert_eq!(&*keys.borrow(), &["p", "x", "enter"]);
                assert!(dock.pending_content_close_plans().unwrap().is_empty());
            }
            shutdown(&dock, cx);
        }
    }
}

#[open_gpui::test]
fn product_menu_retires_an_earlier_list_after_focus_leaves(cx: &mut TestAppContext) {
    use open_gpui::PlatformFrameDeliveryMode;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Handoff {
        Escape,
        AppFocusAfterSwitch,
        AppFocusBeforeSwitch,
    }

    struct Workspace {
        regions: open_gpui::Entity<RegionsRoot>,
        application_input: open_gpui::Entity<InputPane>,
    }

    impl Render for Workspace {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.regions.clone()).child(
                div()
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .size(px(1.0))
                    .child(self.application_input.clone()),
            )
        }
    }

    let sibling = SurfaceId::new(52);
    for list_already_presented in [false, true] {
        for handoff in [
            Handoff::Escape,
            Handoff::AppFocusAfterSwitch,
            Handoff::AppFocusBeforeSwitch,
        ] {
            let (dock, _) = overflowing_surfaces_with_mode(&[SURFACE, sibling], false, cx);
            let keys = Rc::new(RefCell::new(Vec::new()));
            let application_focus = cx.update(|cx| cx.focus_handle());
            cx.defer_next_window_frame_requests();
            let window = cx.open_window(size(px(620.0), px(600.0)), |_, cx| Workspace {
                regions: cx.new(|_| RegionsRoot {
                    dock: dock.clone(),
                    regions: vec![
                        DockRegion::new(SURFACE, LogicalRect::new(0.0, 0.0, 620.0, 290.0).unwrap()),
                        DockRegion::new(
                            sibling,
                            LogicalRect::new(0.0, 300.0, 620.0, 290.0).unwrap(),
                        ),
                    ],
                    scrolls: Rc::default(),
                }),
                application_input: cx.new(|_| InputPane {
                    focus: application_focus.clone(),
                    keys: keys.clone(),
                }),
            });
            let owner = window.into();
            let mut visual = active_visual(owner, cx);
            assert!(visual.activate_accessibility());
            frames(&mut visual, &dock);
            let tree = visual.latest_accessibility_tree_update().unwrap();
            let button_in_region = |label, in_sibling| {
                tree.nodes
                    .iter()
                    .find_map(|(id, node)| {
                        (node.role() == Role::Button
                            && node.label() == Some(label)
                            && node
                                .bounds()
                                .is_some_and(|bounds| (bounds.y0 >= 300.0) == in_sibling))
                        .then_some(*id)
                    })
                    .unwrap()
            };
            let list_opener = button_in_region("Show all tabs", false);
            let target = button_in_region("Tab group actions", true);
            assert!(
                visual.dispatch_accessibility_action(accesskit::ActionRequest {
                    action: AccessibleAction::Focus,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: list_opener,
                    data: None,
                })
            );
            frames(&mut visual, &dock);
            assert_eq!(
                visual.latest_accessibility_tree_update().unwrap().focus,
                list_opener
            );
            if !list_already_presented {
                visual.set_window_frame_delivery_mode(owner, PlatformFrameDeliveryMode::Deferred);
            }
            visual.update(|window, cx| key_pair(window, "down", cx));
            if list_already_presented {
                frames(&mut visual, &dock);
            } else {
                assert!(visual.simulate_frame(RequestFrameOptions {
                    require_presentation: true,
                    force_render: true,
                    ..Default::default()
                }));
            }
            let row = |visual: &VisualTestContext| {
                visual
                    .latest_accessibility_tree_update()
                    .unwrap()
                    .nodes
                    .into_iter()
                    .find_map(|(id, node)| {
                        (node.role() == Role::MenuItem && node.label() == Some("Tab number 3"))
                            .then_some(id)
                    })
            };
            let mut old_row = row(&visual);
            assert_eq!(old_row.is_some(), list_already_presented);
            assert_eq!(
                visual.pending_window_frame_submission(owner).is_none(),
                list_already_presented,
            );

            if handoff == Handoff::AppFocusBeforeSwitch {
                visual.update(|window, cx| {
                    window.focus(&application_focus, cx);
                    key_pair(window, "home", cx);
                });
                assert_eq!(&*keys.borrow(), &["home"]);
            }
            assert!(
                visual.dispatch_accessibility_action(accesskit::ActionRequest {
                    action: AccessibleAction::Click,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: target,
                    data: None,
                })
            );
            let focus_claim = visual.update(|window, cx| match handoff {
                Handoff::Escape => {
                    key_pair(window, "escape", cx);
                    None
                }
                Handoff::AppFocusAfterSwitch => {
                    window.focus(&application_focus, cx);
                    Some(window.focus_claim_revision())
                }
                Handoff::AppFocusBeforeSwitch => None,
            });
            for _ in 0..8 {
                if let Some(id) = old_row.or_else(|| row(&visual)) {
                    old_row = Some(id);
                    visual.dispatch_accessibility_action(accesskit::ActionRequest {
                        action: AccessibleAction::Click,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: id,
                        data: None,
                    });
                }
                if let Some(token) = visual.pending_window_frame_submission(owner) {
                    visual.update(|_, _| {
                        assert!(token.complete(PlatformWindowPresentOutcome::Submitted));
                    });
                    visual.run_until_parked();
                }
                assert!(visual.simulate_frame(RequestFrameOptions {
                    require_presentation: true,
                    force_render: true,
                    ..Default::default()
                }));
                assert!(dock.error().is_none(), "{:?}", dock.error());
                assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
            }
            if let Some(token) = visual.pending_window_frame_submission(owner) {
                visual.update(|_, _| {
                    assert!(token.complete(PlatformWindowPresentOutcome::Submitted));
                });
                visual.run_until_parked();
            }
            visual.set_window_frame_delivery_mode(owner, PlatformFrameDeliveryMode::Synchronous);
            frames(&mut visual, &dock);
            assert!(dock.with_view(|view| view.item(ItemId::new(1)).unwrap().is_active()));
            assert!(
                row(&visual).is_none(),
                "the superseded list must retire ({handoff:?}, presented: {list_already_presented})"
            );
            assert_eq!(
                visual
                    .latest_accessibility_tree_update()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Menu
                        && node.label() == Some("Tab group actions")),
                handoff != Handoff::Escape,
                "{handoff:?}, earlier list presented: {list_already_presented}"
            );
            if handoff == Handoff::AppFocusBeforeSwitch {
                assert_accessibility_focus(&visual, Role::Button, "Tab group actions");
            }
            if let Some(revision) = focus_claim {
                visual.update(|window, cx| {
                    assert!(application_focus.is_focused(window));
                    assert_eq!(window.focus_claim_revision(), revision);
                    key_pair(window, "x", cx);
                });
                assert_eq!(&*keys.borrow(), &["x"]);
            }
            assert!(dock.pending_content_close_plans().unwrap().is_empty());
        }
    }
}

struct InputPane {
    focus: FocusHandle,
    keys: Rc<RefCell<Vec<String>>>,
}

impl Render for InputPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let keys = self.keys.clone();
        div()
            .id("input-pane")
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(move |event, _, _| keys.borrow_mut().push(event.keystroke.key.clone()))
    }
}

#[open_gpui::test]
fn removing_a_product_menu_region_releases_focus_to_a_sibling_pane(cx: &mut TestAppContext) {
    use dockspace::close::CloseDecision;

    let sibling = SurfaceId::new(52);
    let (dock, received) = cx.update(|cx| {
        let first = ItemId::new(1);
        let second = ItemId::new(2);
        let side = ItemId::new(41);
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(
                SURFACE,
                DockspaceRootLayout::new(
                    RootId::new(SURFACE.get()),
                    DockspaceNode::tabs([first, second]),
                ),
            ),
            DockspaceSurfaceLayout::new(
                sibling,
                DockspaceRootLayout::new(RootId::new(sibling.get()), DockspaceNode::tabs([side])),
            ),
        ])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let received = Rc::<RefCell<Vec<String>>>::default();
        let panes = [first, second, side].map(|item| {
            let focus = cx.focus_handle();
            let view = cx.new(|_| InputPane {
                focus: focus.clone(),
                keys: if item == second {
                    received.clone()
                } else {
                    Rc::default()
                },
            });
            DockPane::new(item, format!("Pane {}", item.get()), view).with_focus_handle(focus)
        });
        (
            DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap(),
            received,
        )
    });
    cx.defer_next_window_frame_requests();
    let window = cx.open_window(size(px(620.0), px(600.0)), |_, _| RegionsRoot {
        dock: dock.clone(),
        regions: vec![
            DockRegion::new(SURFACE, LogicalRect::new(0.0, 0.0, 620.0, 290.0).unwrap()),
            DockRegion::new(sibling, LogicalRect::new(0.0, 300.0, 620.0, 290.0).unwrap()),
        ],
        scrolls: Rc::default(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let menu = visual
        .latest_accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button
                && node.label() == Some("Tab group actions")
                && node.bounds().is_some_and(|bounds| bounds.y0 >= 300.0))
            .then_some(id)
        })
        .unwrap();
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: menu,
            data: None,
        })
    );
    frames(&mut visual, &dock);
    assert_eq!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Float Tab Group")
    );
    visual.update(|_, cx| {
        dock.update(
            |frame| {
                frame.request_close_root_current(RootId::new(sibling.get()))?;
                Ok(())
            },
            cx,
        )
        .unwrap();
        let plans = dock.pending_content_close_plans().unwrap();
        let [plan] = plans.as_slice() else {
            panic!("expected one close plan")
        };
        dock.update(
            |frame| {
                for item in plan.items() {
                    frame.resolve_close(plan.request(), item.token(), CloseDecision::Allow)?;
                }
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    assert!(dock.with_view(|view| view.surface(sibling).is_none()));
    tab_list_accessible_action(&visual, Role::Tab, "Pane 1", AccessibleAction::Focus);
    visual.simulate_keystrokes("right");
    frames(&mut visual, &dock);
    assert_accessibility_focus(&visual, Role::Tab, "Pane 2");
    visual.simulate_keystrokes("enter");
    frames(&mut visual, &dock);
    visual.simulate_keystrokes("x");
    assert_eq!(&*received.borrow(), &["x"]);
}

#[open_gpui::test]
fn inactive_accessibility_opener_cannot_replace_a_foreground_tab_list(cx: &mut TestAppContext) {
    let sibling = SurfaceId::new(52);
    for managed in [false, true] {
        let (dock, _) = overflowing_surfaces_with_mode(&[SURFACE, sibling], managed, cx);
        let [first, second] = [SURFACE, sibling].map(|surface| {
            cx.defer_next_window_frame_requests();
            cx.open_window(size(px(620.0), px(400.0)), |window, cx| {
                if managed {
                    dock.bind_native_root(surface, surface == SURFACE, window, cx)
                        .unwrap();
                }
                SingleRegion(dock.clone(), surface)
            })
        });
        let mut source = active_visual(first.into(), cx);
        assert!(source.activate_accessibility());
        frames(&mut source, &dock);
        let opener = source
            .latest_accessibility_tree_update()
            .unwrap()
            .nodes
            .into_iter()
            .find_map(|(id, node)| {
                (node.role() == Role::Button
                    && node.label() == Some("Show all tabs")
                    && !node.is_disabled())
                .then_some(id)
            })
            .unwrap();
        let mut target = active_visual(second.into(), cx);
        assert!(target.activate_accessibility());
        frames(&mut target, &dock);
        tab_list_accessible_action(
            &target,
            Role::Button,
            "Show all tabs",
            AccessibleAction::Click,
        );
        frames(&mut target, &dock);
        assert_accessibility_focus(&target, Role::MenuItem, "Tab number 41");
        assert!(!source.update(|window, _| window.is_window_active()));
        assert!(
            source.dispatch_accessibility_action(accesskit::ActionRequest {
                action: AccessibleAction::Click,
                target_tree: accesskit::TreeId::ROOT,
                target_node: opener,
                data: None,
            })
        );
        frames(&mut source, &dock);
        frames(&mut target, &dock);
        assert_accessibility_focus(&target, Role::MenuItem, "Tab number 41");
        target.simulate_keystrokes("end enter");
        frames(&mut target, &dock);
        assert!(dock.with_view(|view| view.item(ItemId::new(80)).unwrap().is_active()));
        if managed {
            cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
            cx.run_until_parked();
            assert!(dock.native_is_quiescent());
        }
    }
}
