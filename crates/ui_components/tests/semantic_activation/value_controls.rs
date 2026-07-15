use super::*;

#[open_gpui::test]
fn read_only_checkbox_and_toggle_block_every_activation_source(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        checkbox_changes: Rc<RefCell<Vec<Toggled>>>,
        toggle_changes: Rc<RefCell<Vec<bool>>>,
        checkbox_handle: ActivationHandle,
        toggle_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let checkbox_changes = self.checkbox_changes.clone();
            let toggle_changes = self.toggle_changes.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Checkbox::new("read-only-semantic-checkbox")
                        .label("Managed checkbox")
                        .read_only(true)
                        .on_toggle(move |next, _, _| {
                            checkbox_changes.borrow_mut().push(next);
                        })
                        .activation_handle(&self.checkbox_handle),
                )
                .child(
                    div()
                        .debug_selector(|| "read-only-semantic-toggle-hitbox".to_owned())
                        .child(
                            Toggle::new("read-only-semantic-toggle", "Managed toggle")
                                .read_only(true)
                                .on_change(move |next, _, _| {
                                    toggle_changes.borrow_mut().push(next);
                                })
                                .activation_handle(&self.toggle_handle),
                        ),
                )
        }
    }

    let checkbox_changes = Rc::new(RefCell::new(Vec::new()));
    let toggle_changes = Rc::new(RefCell::new(Vec::new()));
    let checkbox_handle = ActivationHandle::new();
    let toggle_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        checkbox_changes: checkbox_changes.clone(),
        toggle_changes: toggle_changes.clone(),
        checkbox_handle: checkbox_handle.clone(),
        toggle_handle: toggle_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let checkbox_state = Checkbox::new("read-only-checkbox-state")
        .read_only(true)
        .state();
    let toggle_state = Toggle::new("read-only-toggle-state", "Managed")
        .read_only(true)
        .state();
    assert!(checkbox_state.read_only());
    assert!(!checkbox_state.activation_enabled());
    assert!(toggle_state.read_only());
    assert!(!toggle_state.activation_enabled());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("read-only value controls should publish a final accessibility tree");
    let checkbox_node = node_with_label(&update, "Managed checkbox");
    let toggle_node = node_with_label(&update, "Managed toggle");
    for node_id in [checkbox_node, toggle_node] {
        let node = update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == node_id).then_some(node))
            .expect("read-only value-control node should exist");
        assert!(node.is_read_only());
        assert!(!node.supports_action(accesskit::Action::Click));
    }

    let checkbox_bounds = cx
        .debug_bounds("checkbox:read-only-semantic-checkbox:root")
        .expect("read-only Checkbox should expose a stable root selector");
    let toggle_bounds = cx
        .debug_bounds("read-only-semantic-toggle-hitbox")
        .expect("read-only Toggle wrapper should expose a hit-test selector");
    cx.simulate_click(checkbox_bounds.center(), Modifiers::none());
    cx.simulate_click(toggle_bounds.center(), Modifiers::none());

    for node_id in [checkbox_node, toggle_node] {
        assert!(
            cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, node_id,))
        );
        let space_down =
            cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
        let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
        assert!(space_down.propagated());
        assert!(!space_down.default_prevented());
        assert!(space_up.propagated());
        assert!(!space_up.default_prevented());
        assert!(
            cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, node_id,))
        );
    }

    cx.update(|window, cx| {
        assert_eq!(
            checkbox_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            toggle_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert!(checkbox_changes.borrow().is_empty());
    assert!(toggle_changes.borrow().is_empty());
}

#[open_gpui::test]
fn value_controls_use_space_only_emit_one_intent_and_honor_read_only(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        switch_value: Rc<RefCell<bool>>,
        switch_changes: Rc<RefCell<Vec<(bool, bool)>>>,
        checkbox_changes: Rc<RefCell<Vec<Toggled>>>,
        toggle_changes: Rc<RefCell<Vec<bool>>>,
        read_only_changes: Rc<RefCell<Vec<bool>>>,
        fallback_clicks: Rc<RefCell<usize>>,
        switch_activation_handle: ActivationHandle,
        read_only_activation_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let current_switch = self.switch_value.clone();
            let switch_changes = self.switch_changes.clone();
            let checkbox_changes = self.checkbox_changes.clone();
            let toggle_changes = self.toggle_changes.clone();
            let read_only_changes = self.read_only_changes.clone();
            let fallback_clicks = self.fallback_clicks.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Switch::new("semantic-switch")
                        .label("Notifications")
                        .checked(*self.switch_value.borrow())
                        .on_change(move |next, _, _| {
                            switch_changes
                                .borrow_mut()
                                .push((next, *current_switch.borrow()));
                        })
                        .activation_handle(&self.switch_activation_handle),
                )
                .child(
                    Checkbox::new("semantic-checkbox")
                        .label("Accept")
                        .on_toggle(move |next, _, _| {
                            checkbox_changes.borrow_mut().push(next);
                        }),
                )
                .child(
                    Toggle::new("semantic-toggle", "Bold").on_change(move |next, _, _| {
                        toggle_changes.borrow_mut().push(next);
                    }),
                )
                .child(
                    div()
                        .id("read-only-fallback-parent")
                        .on_click(move |_, _, _| *fallback_clicks.borrow_mut() += 1)
                        .child(
                            Switch::new("read-only-semantic-switch")
                                .label("Managed")
                                .read_only(true)
                                .on_change(move |next, _, _| {
                                    read_only_changes.borrow_mut().push(next);
                                })
                                .activation_handle(&self.read_only_activation_handle),
                        ),
                )
        }
    }

    let switch_value = Rc::new(RefCell::new(false));
    let switch_changes = Rc::new(RefCell::new(Vec::new()));
    let checkbox_changes = Rc::new(RefCell::new(Vec::new()));
    let toggle_changes = Rc::new(RefCell::new(Vec::new()));
    let read_only_changes = Rc::new(RefCell::new(Vec::new()));
    let fallback_clicks = Rc::new(RefCell::new(0));
    let switch_activation_handle = ActivationHandle::new();
    let read_only_activation_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        switch_value,
        switch_changes: switch_changes.clone(),
        checkbox_changes: checkbox_changes.clone(),
        toggle_changes: toggle_changes.clone(),
        read_only_changes: read_only_changes.clone(),
        fallback_clicks: fallback_clicks.clone(),
        switch_activation_handle: switch_activation_handle.clone(),
        read_only_activation_handle: read_only_activation_handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("value controls should publish a final accessibility tree");
    let switch_node = node_with_label(&update, "Notifications");
    let checkbox_node = node_with_label(&update, "Accept");
    let toggle_node = node_with_label(&update, "Bold");
    let read_only_node = node_with_label(&update, "Managed");
    let read_only = update
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == read_only_node).then_some(node))
        .expect("read-only Switch node should exist");
    assert!(read_only.is_read_only());
    assert!(!read_only.supports_action(accesskit::Action::Click));

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, switch_node,))
    );
    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagated());
    assert!(enter_up.propagated());
    assert!(switch_changes.borrow().is_empty());

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let repeated_space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), true));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert!(repeated_space_down.propagation_stopped());
    assert!(repeated_space_down.default_prevented());
    assert!(switch_changes.borrow().is_empty());

    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(
        switch_changes.borrow().as_slice(),
        &[(true, false)],
        "controlled callbacks must observe the owner's committed value"
    );

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, switch_node,))
    );
    assert_eq!(
        switch_changes.borrow().as_slice(),
        &[(true, false), (true, false)]
    );
    cx.update(|window, cx| {
        assert_eq!(
            switch_activation_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            read_only_activation_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(
        switch_changes.borrow().as_slice(),
        &[(true, false), (true, false), (true, false)]
    );

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, checkbox_node,))
    );
    assert_eq!(checkbox_changes.borrow().as_slice(), &[Toggled::True]);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, toggle_node,))
    );
    cx.simulate_event(key_down("space", Modifiers::none(), false));
    cx.simulate_event(key_up("space", Modifiers::none()));
    assert_eq!(toggle_changes.borrow().as_slice(), &[true]);

    let read_only_bounds = cx
        .debug_bounds("switch:read-only-semantic-switch:root")
        .expect("read-only Switch should keep its root selector");
    cx.simulate_click(read_only_bounds.center(), Modifiers::none());
    let fallback_clicks_before_accessibility = *fallback_clicks.borrow();
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, read_only_node,))
    );
    assert!(read_only_changes.borrow().is_empty());
    assert_eq!(
        *fallback_clicks.borrow(),
        fallback_clicks_before_accessibility,
        "read-only controls must handle AccessKit Click directly instead of falling back to a parent pointer click"
    );
}
