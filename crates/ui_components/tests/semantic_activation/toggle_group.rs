use super::*;

use open_gpui_ui_components::{
    ToggleGroup, ToggleGroupItem, ToggleGroupSelectionChange, ToggleGroupSelectionMode,
};

fn toggled_state(update: &accesskit::TreeUpdate, label: &str) -> Option<accesskit::Toggled> {
    let id = node_with_label(update, label);
    update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then(|| node.toggled()))
        .flatten()
}

#[open_gpui::test]
fn controlled_toggle_group_routes_every_activation_source_without_hidden_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
        right_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            ToggleGroup::new("controlled-semantic-toggle-group", "Alignment")
                .selected_values(Vec::<String>::new())
                .item(ToggleGroupItem::new("left", "Controlled Left"))
                .item(ToggleGroupItem::new("right", "Controlled Right"))
                .item(ToggleGroupItem::new("managed", "Controlled Managed").disabled(true))
                .activation_handle("right", &self.right_handle)
                .activation_handle("managed", &self.disabled_handle)
                .on_change(move |change, _, _| {
                    changes.borrow_mut().push(change.selected_values().to_vec());
                })
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let right_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        right_handle: right_handle.clone(),
        disabled_handle: disabled_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("controlled toggle group should publish a final accessibility tree");
    let right_node = node_with_label(&initial, "Controlled Right");
    let managed_node = node_with_label(&initial, "Controlled Managed");
    let managed = initial
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == managed_node).then_some(node))
        .expect("disabled toggle group item should remain in the final tree");
    assert!(!managed.supports_action(accesskit::Action::Click));

    let right_bounds = cx
        .debug_bounds("toggle-group:controlled-semantic-toggle-group:item:right")
        .expect("right toggle item should expose a stable selector");
    cx.simulate_click(right_bounds.center(), Modifiers::none());
    assert_eq!(changes.borrow().as_slice(), &[vec!["right".to_owned()]]);

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagated());
    assert!(!enter_down.default_prevented());
    assert!(enter_up.propagated());
    assert!(!enter_up.default_prevented());
    assert_eq!(changes.borrow().len(), 1);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(changes.borrow().len(), 1, "Space activates on key-up");
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(changes.borrow().len(), 2);

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", modified));
    assert!(modified_down.propagated());
    assert!(!modified_down.default_prevented());
    assert!(modified_up.propagated());
    assert!(!modified_up.default_prevented());
    assert_eq!(changes.borrow().len(), 2);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, right_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            right_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(
        changes.borrow().as_slice(),
        &[
            vec!["right".to_owned()],
            vec!["right".to_owned()],
            vec!["right".to_owned()],
            vec!["right".to_owned()],
        ]
    );

    cx.update(|window, cx| window.draw(cx).clear());
    let controlled = cx
        .latest_accessibility_tree_update()
        .expect("controlled redraw should preserve caller-owned selection");
    assert_eq!(
        toggled_state(&controlled, "Controlled Right"),
        Some(accesskit::Toggled::False)
    );
    assert!(
        cx.debug_selector_is_focused("toggle-group:controlled-semantic-toggle-group:item:right")
    );
}

#[open_gpui::test]
fn toggle_group_runtime_payload_matches_renderer_neutral_change_resolution(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        changes: Rc<RefCell<Vec<ToggleGroupSelectionChange>>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            ToggleGroup::new("payload-semantic-toggle-group", "Payload alignment")
                .selected_values(["left"])
                .item(ToggleGroupItem::new("left", "Payload Left"))
                .item(ToggleGroupItem::new("right", "Payload Right"))
                .activation_handle("right", &self.handle)
                .on_change(move |change, _, _| changes.borrow_mut().push(change))
        }
    }

    let expected = ToggleGroup::new("payload-model", "Payload alignment")
        .selected_values(["left"])
        .item(ToggleGroupItem::new("left", "Payload Left"))
        .item(ToggleGroupItem::new("right", "Payload Right"))
        .state()
        .selection_change_for_item("right")
        .expect("right should produce a renderer-neutral selection change");
    assert!(!expected.item().selected());
    assert!(!expected.item().focused());
    assert_eq!(expected.selected_values(), &["right".to_owned()]);

    let changes = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(changes.borrow().as_slice(), &[expected]);
}

#[open_gpui::test]
fn uncontrolled_toggle_group_commits_before_callback_reentry(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
        reentered: Rc<Cell<bool>>,
        left_handle: ActivationHandle,
        right_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let reentered = self.reentered.clone();
            let right_handle = self.right_handle.clone();
            ToggleGroup::new("uncontrolled-semantic-toggle-group", "Formatting")
                .mode(ToggleGroupSelectionMode::Multiple)
                .item(ToggleGroupItem::new("left", "Uncontrolled Left"))
                .item(ToggleGroupItem::new("right", "Uncontrolled Right"))
                .activation_handle("left", &self.left_handle)
                .activation_handle("right", &self.right_handle)
                .on_change(move |change, window, cx| {
                    changes.borrow_mut().push(change.selected_values().to_vec());
                    if change.item().value() == "left" && !reentered.replace(true) {
                        assert_eq!(
                            right_handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                    }
                })
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let reentered = Rc::new(Cell::new(false));
    let left_handle = ActivationHandle::new();
    let right_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        reentered: reentered.clone(),
        left_handle: left_handle.clone(),
        right_handle: right_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    cx.update(|window, cx| {
        assert_eq!(
            left_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(
        changes.borrow().as_slice(),
        &[
            vec!["left".to_owned()],
            vec!["left".to_owned(), "right".to_owned()],
        ],
        "reentrant activation must observe the first committed selection"
    );
    assert!(reentered.get());
    assert!(
        cx.debug_selector_is_focused("toggle-group:uncontrolled-semantic-toggle-group:item:right")
    );

    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("uncontrolled reentrant commit should reach the final tree");
    assert_eq!(
        toggled_state(&update, "Uncontrolled Left"),
        Some(accesskit::Toggled::True)
    );
    assert_eq!(
        toggled_state(&update, "Uncontrolled Right"),
        Some(accesskit::Toggled::True)
    );
}

#[open_gpui::test]
fn required_toggle_group_accepts_noop_activation_without_emitting_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        callback_count: Rc<Cell<usize>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let callback_count = self.callback_count.clone();
            ToggleGroup::new("required-semantic-toggle-group", "Required alignment")
                .default_selected_values(["left"])
                .selection_required(true)
                .item(ToggleGroupItem::new("left", "Required Left"))
                .activation_handle("left", &self.handle)
                .on_change(move |_, _, _| callback_count.set(callback_count.get() + 1))
        }
    }

    let callback_count = Rc::new(Cell::new(0));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        callback_count: callback_count.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("required toggle group should publish a final accessibility tree");
    let left_node = node_with_label(&initial, "Required Left");
    assert_eq!(
        toggled_state(&initial, "Required Left"),
        Some(accesskit::Toggled::True)
    );

    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(callback_count.get(), 0);
    assert!(cx.debug_selector_is_focused("toggle-group:required-semantic-toggle-group:item:left"));

    cx.update(|window, cx| window.draw(cx).clear());
    let unchanged = cx
        .latest_accessibility_tree_update()
        .expect("required no-op activation should preserve the final tree");
    assert_eq!(node_with_label(&unchanged, "Required Left"), left_node);
    assert_eq!(
        toggled_state(&unchanged, "Required Left"),
        Some(accesskit::Toggled::True)
    );
}

#[open_gpui::test]
fn toggle_group_roving_focus_skips_disabled_and_space_activates_on_key_up(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
        prevent_navigation: Rc<Cell<bool>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let prevent_navigation = self.prevent_navigation.clone();
            div()
                .capture_key_down(move |event, window, _| {
                    if prevent_navigation.get() && event.keystroke.key == "right" {
                        window.prevent_default();
                    }
                })
                .child(
                    ToggleGroup::new("keyboard-semantic-toggle-group", "Keyboard alignment")
                        .default_selected_values(["left"])
                        .item(ToggleGroupItem::new("left", "Keyboard Left"))
                        .item(ToggleGroupItem::new("center", "Keyboard Center").disabled(true))
                        .item(ToggleGroupItem::new("right", "Keyboard Right"))
                        .on_change(move |change, _, _| {
                            changes.borrow_mut().push(change.selected_values().to_vec());
                        }),
                )
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let prevent_navigation = Rc::new(Cell::new(false));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        prevent_navigation: prevent_navigation.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("keyboard toggle group should publish a final accessibility tree");
    let left_node = node_with_label(&initial, "Keyboard Left");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, left_node,)));

    prevent_navigation.set(true);
    let prevented =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", Modifiers::none(), false));
    prevent_navigation.set(false);
    assert!(prevented.default_prevented());
    assert!(prevented.propagated());
    assert!(cx.debug_selector_is_focused("toggle-group:keyboard-semantic-toggle-group:item:left"));

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_right =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", modified, false));
    assert!(modified_right.propagated());
    assert!(!modified_right.default_prevented());
    assert!(changes.borrow().is_empty());

    let right =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", Modifiers::none(), false));
    assert!(right.propagation_stopped());
    assert!(changes.borrow().is_empty());
    assert!(cx.debug_selector_is_focused("toggle-group:keyboard-semantic-toggle-group:item:right"));

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagated());
    assert!(enter_up.propagated());
    assert!(changes.borrow().is_empty());

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert!(changes.borrow().is_empty());
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(changes.borrow().as_slice(), &[vec!["right".to_owned()]]);

    cx.update(|window, cx| window.draw(cx).clear());
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("Space activation should reach the final accessibility tree");
    assert_eq!(
        toggled_state(&selected, "Keyboard Left"),
        Some(accesskit::Toggled::False)
    );
    assert_eq!(
        toggled_state(&selected, "Keyboard Right"),
        Some(accesskit::Toggled::True)
    );
}
