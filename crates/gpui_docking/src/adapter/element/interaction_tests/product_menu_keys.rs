use dockspace::policy::CloseCapability;
use open_gpui::{
    Keystroke, MouseDownEvent, MouseUpEvent, PlatformInput, Point, ScrollDelta, ScrollEvent,
    ScrollSource, ScrollWheelIntent, TouchPhase, point,
};

use super::*;

fn focus_layout_actions(dock: &DockSurface, visual: &mut VisualTestContext) {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let target = tree
        .nodes
        .iter()
        .find_map(|(target, node)| {
            (node.role() == Role::Button && node.label() == Some("Layout actions"))
                .then_some(*target)
        })
        .expect("the closed layout menu must expose its opener");
    assert!(
        visual.dispatch_accessibility_action(accesskit::ActionRequest {
            action: AccessibleAction::Focus,
            target_tree: accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        })
    );
    frames(visual, dock);
    assert_accessibility_focus(visual, Role::Button, "Layout actions");
    assert!(!presentation_menu_is_open(visual));
}

fn semantic_center(visual: &mut VisualTestContext, role: Role, label: &str) -> Point<Pixels> {
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let node = tree
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == role && node.label() == Some(label)).then_some(node))
        .expect("the published tree must expose the requested control");
    assert!(!node.is_disabled(), "{label} must be enabled");
    let bounds = node.bounds().expect("the real control must have bounds");
    let scale = visual.update(|window, _| f64::from(window.scale_factor()));
    point(
        px(((bounds.x0 + bounds.x1) * 0.5 / scale) as f32),
        px(((bounds.y0 + bounds.y1) * 0.5 / scale) as f32),
    )
}

#[open_gpui::test]
fn closed_product_menu_down_enter_waits_for_presentation_without_losing_confirmation(
    cx: &mut TestAppContext,
) {
    let (dock, _, mut visual, _) = presentation_menu_with_content_focus(cx);
    focus_layout_actions(&dock, &mut visual);
    let window = visual.update(|window, _| window.window_handle());
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    visual.update(|window, cx| {
        for key in ["down", "enter"] {
            assert!(
                window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx),
                "the opener must retain the ordered {key} intent"
            );
        }
        assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
    });
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        for item in [FIRST, SECOND] {
            assert!(
                view.item(item).unwrap().contained().is_none(),
                "an unpresented menu must not authorize confirmation"
            );
        }
    });
    assert!(dock.pending_content_close_plans().unwrap().is_empty());

    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        let contained = view.item(FIRST).unwrap().contained();
        assert!(
            contained.is_some(),
            "the retained Enter must activate Float Layout"
        );
        assert_eq!(view.item(SECOND).unwrap().contained(), contained);
        assert_eq!(view.surface(SURFACE).unwrap().contained_count(), 1);
    });
    assert!(!presentation_menu_is_open(&visual));
}

#[open_gpui::test]
fn closed_product_menu_up_selects_the_last_enabled_row(cx: &mut TestAppContext) {
    let (dock, _, mut visual, _) = presentation_menu_with_content_focus(cx);
    focus_layout_actions(&dock, &mut visual);
    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("up").unwrap(), cx));
    });
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert_eq!(
        selected_presentation_menu_row(&visual).as_deref(),
        Some("Close Layout")
    );
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let selected = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::MenuItem && node.is_selected() == Some(true))
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert!(!selected[0].1.is_disabled());
    assert!(dock.pending_content_close_plans().unwrap().is_empty());
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
}

#[open_gpui::test]
fn cancelled_product_menu_press_cannot_activate_a_reopened_menu(cx: &mut TestAppContext) {
    let (dock, _, mut visual, anchor) = presentation_menu_with_content_focus(cx);
    visual.simulate_click(anchor.center(), Modifiers::none());
    frames(&mut visual, &dock);
    let position = semantic_center(&mut visual, Role::MenuItem, "Float Layout");

    visual.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        for key in ["escape", "enter"] {
            assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
        }
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
        assert!(
            dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()),
            "the cancelled press must not activate the reopened menu's command"
        );
    });
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));

    let position = semantic_center(&mut visual, Role::MenuItem, "Float Layout");
    visual.simulate_click(position, Modifiers::none());
    frames(&mut visual, &dock);
    dock.with_view(|view| {
        let contained = view.item(FIRST).unwrap().contained();
        assert!(
            contained.is_some(),
            "a fresh click must still activate Float Layout"
        );
        assert_eq!(view.item(SECOND).unwrap().contained(), contained);
    });
    assert!(!presentation_menu_is_open(&visual));
}

#[open_gpui::test]
fn product_menu_with_no_enabled_rows_has_no_selection_and_still_dismisses(cx: &mut TestAppContext) {
    let (dock, _, mut visual, _) = presentation_menu_with_content_focus(cx);
    visual.update(|_, cx| {
        let mut policy = DockPolicy::default();
        policy.set_allow_contained_floating(false);
        policy.set_allow_native_surfaces(false);
        policy.set_close_capability(CloseCapability::Disabled);
        dock.update(
            |frame| {
                frame.replace_policy(policy)?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    frames(&mut visual, &dock);
    let anchor = semantic_center(&mut visual, Role::Button, "Layout actions");
    visual.simulate_click(anchor, Modifiers::none());
    frames(&mut visual, &dock);
    assert!(presentation_menu_is_open(&visual));
    assert_eq!(selected_presentation_menu_row(&visual), None);

    visual.update(|window, cx| {
        for key in ["down", "up", "home", "end", "enter"] {
            let _ = window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
        }
    });
    frames(&mut visual, &dock);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let entries = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::MenuItem)
        .collect::<Vec<_>>();
    assert!(
        !entries.is_empty(),
        "the test must exercise visible disabled rows"
    );
    for (_, node) in entries {
        assert!(node.is_disabled());
        assert_ne!(node.is_selected(), Some(true));
        assert!(!node.supports_action(AccessibleAction::Click));
    }
    assert!(dock.pending_content_close_plans().unwrap().is_empty());
    dock.with_view(|view| {
        for item in [FIRST, SECOND] {
            assert!(view.item(item).unwrap().contained().is_none());
        }
    });

    visual.update(|window, cx| {
        assert!(window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx));
    });
    frames(&mut visual, &dock);
    assert!(!presentation_menu_is_open(&visual));
    assert_accessibility_focus(&visual, Role::Button, "Layout actions");
}

struct ScrollPane(Rc<Cell<usize>>);

impl Render for ScrollPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scrolls = self.0.clone();
        div()
            .id("product-menu-scroll-pane")
            .debug_selector(|| "product-menu-scroll-pane".to_owned())
            .size_full()
            .on_scroll(move |_, _, _| {
                scrolls.set(scrolls.get() + 1);
                ScrollWheelIntent::handled().stop_propagation()
            })
    }
}

#[open_gpui::test]
fn product_menu_occludes_wheel_events_from_the_underlying_pane(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (dock, scrolls) = cx.update(|cx| {
        let scrolls = Rc::new(Cell::new(0));
        let panes = [FIRST, SECOND].map(|item| {
            let view = cx.new(|_| ScrollPane(scrolls.clone()));
            DockPane::new(item, format!("Pane {}", item.get()), view)
        });
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(RootId::new(71), DockspaceNode::tabs([FIRST, SECOND])),
        )])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let dock = DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap();
        (dock, scrolls)
    });
    let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root {
        dock: dock.clone(),
    });
    let mut visual = active_visual(window.into(), cx);
    assert!(visual.activate_accessibility());
    frames(&mut visual, &dock);
    let pane = visual.debug_bounds("product-menu-scroll-pane").unwrap();
    visual.simulate_event(ScrollEvent {
        position: pane.center(),
        delta: ScrollDelta::Lines(point(0.0, -3.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    frames(&mut visual, &dock);
    assert_eq!(
        scrolls.get(),
        1,
        "the pane probe must receive an uncovered wheel event"
    );

    let anchor = semantic_center(&mut visual, Role::Button, "Layout actions");
    visual.simulate_click(anchor, Modifiers::none());
    frames(&mut visual, &dock);
    let position = semantic_center(&mut visual, Role::MenuItem, "Float Layout");
    assert!(
        pane.contains(&position),
        "the menu must cover the real pane probe"
    );
    for (delta, touch_phase) in [
        (ScrollDelta::Lines(point(0.0, -3.0)), TouchPhase::Moved),
        (
            ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
            TouchPhase::Started,
        ),
        (ScrollDelta::default(), TouchPhase::Ended),
    ] {
        visual.simulate_event(ScrollEvent {
            position,
            delta,
            modifiers: Modifiers::none(),
            touch_phase,
            source: ScrollSource::Pointer,
        });
    }
    frames(&mut visual, &dock);
    assert_eq!(
        scrolls.get(),
        1,
        "the popup must occlude the covered pane's wheel handlers"
    );
    assert!(presentation_menu_is_open(&visual));
    assert!(dock.with_view(|view| view.item(FIRST).unwrap().contained().is_none()));
}
