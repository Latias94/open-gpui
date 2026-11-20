use open_gpui::{KeyUpEvent, Keystroke, PlatformInput};

use super::*;

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
