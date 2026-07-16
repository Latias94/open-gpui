use super::*;

use open_gpui_ui_components::toolbar::ToolbarItem;
use open_gpui_ui_components::{Toolbar, ToolbarActivation, ToolbarItemKind};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedActivation {
    value: String,
    kind: ToolbarItemKind,
    pressed: bool,
    source: ActivationSource,
}

impl ObservedActivation {
    fn new(activation: &ToolbarActivation, input: open_gpui_ui_components::Activation) -> Self {
        Self {
            value: activation.value().to_owned(),
            kind: activation.kind(),
            pressed: activation.pressed(),
            source: input.source(),
        }
    }
}

#[open_gpui::test]
fn toolbar_action_routes_every_activation_source_through_one_transaction(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<ObservedActivation>>>,
        save_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
        duplicate_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Toolbar::new("semantic-toolbar", "Editor actions")
                .default_focused("save")
                .item(ToolbarItem::action("save", "Save"))
                .item(ToolbarItem::action("disabled", "Disabled action").disabled(true))
                .item(ToolbarItem::action("duplicate", "Duplicate action"))
                .item(ToolbarItem::toggle("duplicate", "Duplicate toggle"))
                .activation_handle("save", &self.save_handle)
                .activation_handle("disabled", &self.disabled_handle)
                .activation_handle("duplicate", &self.duplicate_handle)
                .on_activate(move |activation, input, _, _| {
                    activations
                        .borrow_mut()
                        .push(ObservedActivation::new(&activation, input));
                })
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let save_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let duplicate_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        save_handle: save_handle.clone(),
        disabled_handle: disabled_handle.clone(),
        duplicate_handle: duplicate_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("toolbar should publish a final accessibility tree");
    let save_node = node_with_label(&initial, "Save");
    let disabled_node = node_with_label(&initial, "Disabled action");
    let duplicate_action_node = node_with_label(&initial, "Duplicate action");
    let duplicate_toggle_node = node_with_label(&initial, "Duplicate toggle");
    assert_ne!(duplicate_action_node, duplicate_toggle_node);
    assert!(
        cx.debug_bounds("toolbar:semantic-toolbar:duplicate-item:2:duplicate")
            .is_some()
    );
    assert!(
        cx.debug_bounds("toolbar:semantic-toolbar:duplicate-item:3:duplicate")
            .is_some()
    );
    let disabled = initial
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == disabled_node).then_some(node))
        .expect("disabled toolbar action should remain in the final tree");
    assert!(!disabled.supports_action(accesskit::Action::Click));
    for duplicate_node in [duplicate_action_node, duplicate_toggle_node] {
        let duplicate = initial
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == duplicate_node).then_some(node))
            .expect("duplicate toolbar item should remain visible in the final tree");
        assert!(duplicate.is_disabled());
        assert!(!duplicate.supports_action(accesskit::Action::Click));
    }

    let save_bounds = cx
        .debug_bounds("toolbar:semantic-toolbar:item:save")
        .expect("save action should expose a stable selector");
    cx.simulate_click(save_bounds.center(), Modifiers::none());
    assert_eq!(activations.borrow().len(), 1);
    assert!(cx.debug_selector_is_focused("toolbar:semantic-toolbar:item:save"));

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert!(!enter_down.default_prevented());
    assert_eq!(activations.borrow().len(), 1, "Enter activates on key-up");
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert!(!enter_up.default_prevented());
    assert_eq!(activations.borrow().len(), 2);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(activations.borrow().len(), 2, "Space activates on key-up");
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(activations.borrow().len(), 3);

    let repeat_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), true));
    let repeat_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(repeat_down.propagated());
    assert!(repeat_up.propagated());
    assert_eq!(activations.borrow().len(), 3, "unarmed repeats are ignored");

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", modified));
    assert!(modified_down.propagated());
    assert!(modified_up.propagated());
    assert_eq!(activations.borrow().len(), 3);

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, save_node,)));
    cx.update(|window, cx| {
        assert_eq!(
            save_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            duplicate_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ObservedActivation {
                value: "save".to_owned(),
                kind: ToolbarItemKind::Action,
                pressed: false,
                source: ActivationSource::Pointer,
            },
            ObservedActivation {
                value: "save".to_owned(),
                kind: ToolbarItemKind::Action,
                pressed: false,
                source: ActivationSource::Keyboard(ActivationKey::Enter),
            },
            ObservedActivation {
                value: "save".to_owned(),
                kind: ToolbarItemKind::Action,
                pressed: false,
                source: ActivationSource::Keyboard(ActivationKey::Space),
            },
            ObservedActivation {
                value: "save".to_owned(),
                kind: ToolbarItemKind::Action,
                pressed: false,
                source: ActivationSource::Accessibility,
            },
            ObservedActivation {
                value: "save".to_owned(),
                kind: ToolbarItemKind::Action,
                pressed: false,
                source: ActivationSource::Programmatic,
            },
        ]
    );
}

#[open_gpui::test]
fn toolbar_toggle_is_space_only_and_item_handler_overrides_toolbar_fallback(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        item_activations: Rc<RefCell<Vec<ObservedActivation>>>,
        toolbar_activations: Rc<RefCell<Vec<ObservedActivation>>>,
        toggle_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item_activations = self.item_activations.clone();
            let toolbar_activations = self.toolbar_activations.clone();
            Toolbar::new("toggle-semantic-toolbar", "Formatting")
                .default_focused("bold")
                .item(
                    ToolbarItem::toggle("bold", "Bold")
                        .pressed(true)
                        .on_activate(move |activation, input, _, _| {
                            item_activations
                                .borrow_mut()
                                .push(ObservedActivation::new(&activation, input));
                        }),
                )
                .activation_handle("bold", &self.toggle_handle)
                .on_activate(move |activation, input, _, _| {
                    toolbar_activations
                        .borrow_mut()
                        .push(ObservedActivation::new(&activation, input));
                })
        }
    }

    let item_activations = Rc::new(RefCell::new(Vec::new()));
    let toolbar_activations = Rc::new(RefCell::new(Vec::new()));
    let toggle_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        item_activations: item_activations.clone(),
        toolbar_activations: toolbar_activations.clone(),
        toggle_handle: toggle_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("toggle toolbar should publish a final accessibility tree");
    let bold_node = node_with_label(&initial, "Bold");

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, bold_node,)));
    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagated());
    assert!(!enter_down.default_prevented());
    assert!(enter_up.propagated());
    assert!(item_activations.borrow().is_empty());

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert!(item_activations.borrow().is_empty());
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());

    assert_eq!(item_activations.borrow().len(), 1);
    assert!(toolbar_activations.borrow().is_empty());
    assert_eq!(item_activations.borrow()[0].value, "bold");
    assert!(
        item_activations.borrow()[0].pressed,
        "payload reports the caller-owned state before activation"
    );

    cx.update(|window, cx| {
        assert_eq!(
            toggle_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });
    assert_eq!(item_activations.borrow().len(), 2);
    assert!(toolbar_activations.borrow().is_empty());

    let controlled = cx
        .latest_accessibility_tree_update()
        .expect("caller-owned toggle state should remain published after activation");
    let bold = controlled
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == bold_node).then_some(node))
        .expect("stable bold node should remain in the final tree");
    assert_eq!(bold.toggled(), Some(accesskit::Toggled::True));
}

#[open_gpui::test]
fn toolbar_reentrant_activation_keeps_the_newest_focus_claim(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        trace: Rc<RefCell<Vec<String>>>,
        first_handle: ActivationHandle,
        second_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let trace = self.trace.clone();
            let second_handle = self.second_handle.clone();
            Toolbar::new("reentrant-semantic-toolbar", "Reentrant actions")
                .default_focused("first")
                .item(ToolbarItem::action("first", "First"))
                .item(ToolbarItem::action("second", "Second"))
                .activation_handle("first", &self.first_handle)
                .activation_handle("second", &self.second_handle)
                .on_activate(move |activation, _, window, cx| {
                    trace.borrow_mut().push(activation.value().to_owned());
                    if activation.value() == "first" {
                        assert_eq!(
                            second_handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                    }
                })
        }
    }

    let trace = Rc::new(RefCell::new(Vec::new()));
    let first_handle = ActivationHandle::new();
    let second_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        trace: trace.clone(),
        first_handle: first_handle.clone(),
        second_handle: second_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("reentrant toolbar should publish a final accessibility tree");
    let second_node = node_with_label(&initial, "Second");

    cx.update(|window, cx| {
        assert_eq!(
            first_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });

    assert_eq!(trace.borrow().as_slice(), &["first", "second"]);
    assert!(cx.debug_selector_is_focused("toolbar:reentrant-semantic-toolbar:item:second"));
    let reentered = cx
        .latest_accessibility_tree_update()
        .expect("reentrant focus should reach the final accessibility tree");
    assert_eq!(reentered.focus, second_node);
}

#[open_gpui::test]
fn toolbar_kind_change_cancels_an_in_flight_activation(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        toggle: bool,
        activations: Rc<RefCell<Vec<ToolbarItemKind>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item = if self.toggle {
                ToolbarItem::toggle("mode", "Mode")
            } else {
                ToolbarItem::action("mode", "Mode")
            };
            let activations = self.activations.clone();
            Toolbar::new("kind-changing-toolbar", "Mode actions")
                .default_focused("mode")
                .item(item)
                .on_activate(move |activation, _, _, _| {
                    activations.borrow_mut().push(activation.kind());
                })
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        toggle: false,
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("kind-changing toolbar should publish a final tree");
    let mode_node = node_with_label(&initial, "Mode");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, mode_node,)));

    let down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(down.propagation_stopped());
    assert!(down.default_prevented());

    view.update(cx, |view, cx| {
        view.toggle = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let stale_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(stale_up.propagated());
    assert!(activations.borrow().is_empty());

    let fresh_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let fresh_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(fresh_down.propagation_stopped());
    assert!(fresh_up.propagation_stopped());
    assert_eq!(activations.borrow().as_slice(), &[ToolbarItemKind::Toggle]);
}

#[open_gpui::test]
fn toolbar_duplicate_identity_namespace_cannot_collide_with_reserved_looking_unique_values(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Toolbar::new("identity-collision-toolbar", "Identity collision toolbar")
                .item(ToolbarItem::action("foo", "Duplicate foo action"))
                .item(ToolbarItem::toggle("foo", "Duplicate foo toggle"))
                .item(ToolbarItem::action(
                    "foo-occurrence-1",
                    "Unique hyphen occurrence",
                ))
                .item(ToolbarItem::action(
                    "foo:occurrence:1",
                    "Unique colon occurrence",
                ))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("identity collision toolbar should publish a final accessibility tree");
    let duplicate_action = node_with_label(&update, "Duplicate foo action");
    let duplicate_toggle = node_with_label(&update, "Duplicate foo toggle");
    let unique_hyphen = node_with_label(&update, "Unique hyphen occurrence");
    let unique_colon = node_with_label(&update, "Unique colon occurrence");

    assert_ne!(duplicate_action, duplicate_toggle);
    assert_ne!(duplicate_action, unique_hyphen);
    assert_ne!(duplicate_action, unique_colon);
    assert_ne!(duplicate_toggle, unique_hyphen);
    assert_ne!(duplicate_toggle, unique_colon);
    assert_ne!(unique_hyphen, unique_colon);

    for selector in [
        "toolbar:identity-collision-toolbar:duplicate-item:0:foo",
        "toolbar:identity-collision-toolbar:duplicate-item:1:foo",
        "toolbar:identity-collision-toolbar:item:foo-occurrence-1",
        "toolbar:identity-collision-toolbar:item:foo:occurrence:1",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "toolbar item should expose the disjoint selector `{selector}`"
        );
    }
}

#[open_gpui::test]
fn toolbar_unique_to_duplicate_redraw_transfers_focus_and_skips_duplicate_items(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        duplicate_middle_values: bool,
        middle_handle: ActivationHandle,
        activations: Rc<RefCell<Vec<(String, ActivationSource)>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let toolbar = Toolbar::new("duplicate-redraw-toolbar", "Duplicate redraw toolbar")
                .default_focused("left")
                .item(ToolbarItem::action("left", "Left"));
            let toolbar = if self.duplicate_middle_values {
                toolbar
                    .item(ToolbarItem::action("middle-b", "Middle first"))
                    .item(ToolbarItem::action("middle-b", "Middle second"))
            } else {
                toolbar
                    .item(ToolbarItem::action("middle-a", "Middle first"))
                    .item(ToolbarItem::action("middle-b", "Middle second"))
            };
            let activations = self.activations.clone();

            toolbar
                .activation_handle("middle-b", &self.middle_handle)
                .item(ToolbarItem::action("right", "Right"))
                .on_activate(move |activation, input, _, _| {
                    activations
                        .borrow_mut()
                        .push((activation.value().to_owned(), input.source()));
                })
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let middle_handle = ActivationHandle::new();
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        duplicate_middle_values: false,
        middle_handle: middle_handle.clone(),
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("unique-value toolbar should publish a final accessibility tree");
    let left_node = node_with_label(&initial, "Left");
    let middle_node = node_with_label(&initial, "Middle second");

    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });
    assert!(cx.debug_selector_is_focused("toolbar:duplicate-redraw-toolbar:item:middle-b"));
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("programmatic focus should reach the accessibility tree")
            .focus,
        middle_node
    );
    assert_eq!(
        activations.borrow().as_slice(),
        &[("middle-b".to_owned(), ActivationSource::Programmatic)]
    );

    view.update(cx, |view, cx| {
        view.duplicate_middle_values = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    for selector in [
        "toolbar:duplicate-redraw-toolbar:duplicate-item:1:middle-b",
        "toolbar:duplicate-redraw-toolbar:duplicate-item:2:middle-b",
    ] {
        assert!(cx.debug_bounds(selector).is_some());
        assert!(
            !cx.debug_selector_is_focused(selector),
            "fail-closed duplicate item `{selector}` must not retain physical focus"
        );
    }
    assert!(
        cx.debug_bounds("toolbar:duplicate-redraw-toolbar:item:middle-b")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("toolbar:duplicate-redraw-toolbar:item:left"));
    let duplicate_update = cx
        .latest_accessibility_tree_update()
        .expect("duplicate redraw should publish a final accessibility tree");
    let resolved_left_node = node_with_label(&duplicate_update, "Left");
    assert_eq!(resolved_left_node, left_node);
    assert_eq!(duplicate_update.focus, resolved_left_node);
    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    let right_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", Modifiers::none(), false));
    let right_up = cx.simulate_event_with_dispatch_snapshot(key_up("right", Modifiers::none()));
    assert!(right_down.propagation_stopped());
    assert!(right_up.propagated());
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.debug_selector_is_focused("toolbar:duplicate-redraw-toolbar:item:right"));
    for selector in [
        "toolbar:duplicate-redraw-toolbar:duplicate-item:1:middle-b",
        "toolbar:duplicate-redraw-toolbar:duplicate-item:2:middle-b",
    ] {
        assert!(!cx.debug_selector_is_focused(selector));
    }
    let navigated = cx
        .latest_accessibility_tree_update()
        .expect("right-arrow navigation should reach the accessibility tree");
    assert_eq!(navigated.focus, node_with_label(&navigated, "Right"));
    assert_eq!(
        activations
            .borrow()
            .iter()
            .filter(|(value, _)| value == "right")
            .count(),
        0
    );

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagation_stopped());
    assert!(enter_up.propagation_stopped());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ("middle-b".to_owned(), ActivationSource::Programmatic),
            (
                "right".to_owned(),
                ActivationSource::Keyboard(ActivationKey::Enter),
            ),
        ]
    );
}

#[open_gpui::test]
fn toolbar_duplicate_redraw_does_not_steal_external_focus(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        duplicate_middle_value: bool,
        middle_handle: ActivationHandle,
        outside_focus: open_gpui::FocusHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let toolbar = Toolbar::new("external-focus-toolbar", "External focus toolbar")
                .default_focused("left")
                .item(ToolbarItem::action("left", "Left"));
            let toolbar = if self.duplicate_middle_value {
                toolbar
                    .item(ToolbarItem::action("middle-b", "Middle first"))
                    .item(ToolbarItem::action("middle-b", "Middle second"))
            } else {
                toolbar
                    .item(ToolbarItem::action("middle-a", "Middle first"))
                    .item(ToolbarItem::action("middle-b", "Middle second"))
            };

            div()
                .child(toolbar.activation_handle("middle-b", &self.middle_handle))
                .child(
                    div()
                        .id("toolbar-outside-focus")
                        .debug_selector(|| "toolbar-outside-focus".to_owned())
                        .role(accesskit::Role::Button)
                        .aria_label("Outside focus")
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.outside_focus)
                        .child("Outside focus"),
                )
        }
    }

    let middle_handle = ActivationHandle::new();
    let (view, cx) = cx.add_window_view(|_, cx| Probe {
        duplicate_middle_value: false,
        middle_handle: middle_handle.clone(),
        outside_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("external-focus toolbar should publish a final accessibility tree");
    let outside_node = node_with_label(&initial, "Outside focus");

    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });
    assert!(cx.debug_selector_is_focused("toolbar:external-focus-toolbar:item:middle-b"));

    cx.update(|window, cx| {
        let outside_focus = view.read(cx).outside_focus.clone();
        outside_focus.focus(window, cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("toolbar-outside-focus"));
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("external focus should reach the accessibility tree")
            .focus,
        outside_node
    );

    view.update(cx, |probe, cx| {
        probe.duplicate_middle_value = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(cx.debug_selector_is_focused("toolbar-outside-focus"));
    let duplicate_update = cx
        .latest_accessibility_tree_update()
        .expect("duplicate redraw should preserve the external accessibility focus");
    assert_eq!(duplicate_update.focus, outside_node);
    assert!(!cx.debug_selector_is_focused("toolbar:external-focus-toolbar:item:left"));
    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert!(cx.debug_selector_is_focused("toolbar-outside-focus"));
}
