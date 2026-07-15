use super::*;

#[open_gpui::test]
fn button_routes_pointer_keyboard_and_accessibility_through_one_typed_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<ActivationSource>>>,
        disabled_activations: Rc<RefCell<Vec<ActivationSource>>>,
        prevent_key_up: Rc<Cell<bool>>,
        stop_key_up: Rc<Cell<bool>>,
        activation_handle: ActivationHandle,
        disabled_activation_handle: ActivationHandle,
        disabled_control: bool,
        show_disabled_control: bool,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let disabled_activations = self.disabled_activations.clone();
            let prevent_key_up = self.prevent_key_up.clone();
            let stop_key_up = self.stop_key_up.clone();

            div()
                .id("semantic-activation-capture-owner")
                .capture_key_up(move |_, window, cx| {
                    if prevent_key_up.get() {
                        window.prevent_default();
                    }
                    if stop_key_up.get() {
                        cx.stop_propagation();
                    }
                })
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Button::new("semantic-activation-button", "Run")
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.source());
                        })
                        .activation_handle(&self.activation_handle),
                )
                .when(self.show_disabled_control, |this| {
                    this.child(
                        Button::new("disabled-semantic-activation-button", "Disabled")
                            .disabled(self.disabled_control)
                            .on_activate(move |activation, _, _| {
                                disabled_activations.borrow_mut().push(activation.source());
                            })
                            .activation_handle(&self.disabled_activation_handle),
                    )
                })
                .child(Button::new("semantic-activation-other-button", "Other"))
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let disabled_activations = Rc::new(RefCell::new(Vec::new()));
    let prevent_key_up = Rc::new(Cell::new(false));
    let stop_key_up = Rc::new(Cell::new(false));
    let activation_handle = ActivationHandle::new();
    let disabled_activation_handle = ActivationHandle::new();
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        disabled_activations: disabled_activations.clone(),
        prevent_key_up: prevent_key_up.clone(),
        stop_key_up: stop_key_up.clone(),
        activation_handle: activation_handle.clone(),
        disabled_activation_handle: disabled_activation_handle.clone(),
        disabled_control: true,
        show_disabled_control: true,
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let button_bounds = cx
        .debug_bounds("button:semantic-activation-button:root")
        .expect("Button should expose a stable root selector");
    cx.simulate_click(button_bounds.center(), Modifiers::none());
    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Pointer]
    );

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Button should publish a final accessibility tree");
    let button_node = node_with_label(&update, "Run");
    let disabled_node = node_with_label(&update, "Disabled");
    let other_node = node_with_label(&update, "Other");
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, button_node,))
    );

    let unpaired_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(unpaired_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "an unpaired key-up must not activate"
    );

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let released_modifier_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(modified_enter_down.propagated());
    assert!(released_modifier_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a modified key-down must not become an activation when the modifier is released first"
    );

    let prevented_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(prevented_enter_down.propagation_stopped());
    prevent_key_up.set(true);
    let prevented_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    prevent_key_up.set(false);
    assert!(prevented_enter_up.propagated());
    assert!(prevented_enter_up.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a capture owner that prevents key-up must cancel activation"
    );

    let stopped_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(stopped_enter_down.propagation_stopped());
    stop_key_up.set(true);
    let stopped_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    stop_key_up.set(false);
    assert!(stopped_enter_up.propagation_stopped());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a capture owner that stops key-up must cancel activation"
    );

    let stale_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(stale_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a later unpaired key-up must not reuse an armed transaction whose release was stopped"
    );

    let focus_changed_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(focus_changed_enter_down.propagation_stopped());
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, other_node,))
    );
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, button_node,))
    );
    let focus_changed_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(focus_changed_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "focus changes must invalidate an armed key transaction"
    );

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert!(!enter_down.default_prevented());
    assert_eq!(activations.borrow().len(), 1, "key-down must not activate");
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert!(!enter_up.default_prevented());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
        ]
    );

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        2,
        "Space key-down must not activate"
    );

    let repeated_space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), true));
    assert!(repeated_space_down.propagation_stopped());
    assert!(repeated_space_down.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        2,
        "held-key repeats must not activate"
    );

    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Keyboard(ActivationKey::Space),
        ]
    );

    let modified_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", modified));
    assert!(modified_down.propagated());
    assert!(!modified_down.default_prevented());
    assert!(modified_up.propagated());
    assert!(!modified_up.default_prevented());
    assert_eq!(activations.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, button_node,))
    );
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Keyboard(ActivationKey::Space),
            ActivationSource::Accessibility,
        ],
        "AccessKit Click must dispatch directly instead of synthesizing a pointer activation"
    );

    cx.update(|window, cx| {
        assert_eq!(
            activation_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(
        activations.borrow().last(),
        Some(&ActivationSource::Programmatic)
    );

    let disabled_bounds = cx
        .debug_bounds("button:disabled-semantic-activation-button:root")
        .expect("disabled Button should keep a stable root selector");
    cx.simulate_click(disabled_bounds.center(), Modifiers::none());
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, disabled_node,))
    );
    assert!(disabled_activations.borrow().is_empty());

    cx.simulate_mouse_down(
        disabled_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    view.update(cx, |probe, cx| {
        probe.disabled_control = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let enabled_bounds = cx
        .debug_bounds("button:disabled-semantic-activation-button:root")
        .expect("enabled Button should retain its stable root selector");
    cx.simulate_mouse_up(
        enabled_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(
        disabled_activations.borrow().is_empty(),
        "a pointer press that began while disabled must not activate after a gate change"
    );
    cx.update(|window, cx| {
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(
        disabled_activations.borrow().as_slice(),
        &[ActivationSource::Programmatic]
    );

    view.update(cx, |probe, cx| {
        probe.show_disabled_control = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
}

#[open_gpui::test]
fn pointer_activation_survives_same_gate_owner_rerender(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        activations: Rc<RefCell<Vec<ActivationSource>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Button::new("pointer-rerender-button", "Continue").on_activate(
                move |activation, _, _| activations.borrow_mut().push(activation.source()),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let bounds = cx
        .debug_bounds("button:pointer-rerender-button:root")
        .expect("Button should expose a stable root selector");
    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let bounds = cx
        .debug_bounds("button:pointer-rerender-button:root")
        .expect("Button should retain its root selector after rerender");
    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());

    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Pointer]
    );
}
