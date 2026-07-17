use super::*;

use open_gpui_ui_components::{Listbox, ListboxGroup, ListboxSelection, listbox::ListboxOption};

fn selected_state(update: &accesskit::TreeUpdate, label: &str) -> Option<bool> {
    let id = node_with_label(update, label);
    update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then_some(node.is_selected()))
        .flatten()
}

#[open_gpui::test]
fn listbox_routes_every_activation_entry_through_one_selection_transaction(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        fallback: Rc<RefCell<Vec<String>>>,
        item: Rc<RefCell<Vec<String>>>,
        beta_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
        duplicate_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let fallback = self.fallback.clone();
            let item = self.item.clone();

            Listbox::new("semantic-listbox", "Semantic listbox")
                .default_selected("alpha")
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::new("beta", "Beta"))
                .option(
                    ListboxOption::new("gamma", "Gamma").on_select(move |selection, _, _| {
                        item.borrow_mut().push(selection.value().to_owned())
                    }),
                )
                .option(ListboxOption::new("disabled", "Disabled").disabled(true))
                .group(
                    ListboxGroup::new("duplicates", "Duplicates")
                        .option(ListboxOption::new("shared", "Duplicate A"))
                        .option(ListboxOption::new("shared", "Duplicate B")),
                )
                .activation_handle("beta", &self.beta_handle)
                .activation_handle("disabled", &self.disabled_handle)
                .activation_handle("shared", &self.duplicate_handle)
                .on_select(move |selection, _, _| {
                    fallback.borrow_mut().push(selection.value().to_owned());
                })
        }
    }

    let fallback = Rc::new(RefCell::new(Vec::new()));
    let item = Rc::new(RefCell::new(Vec::new()));
    let beta_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let duplicate_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        fallback: fallback.clone(),
        item: item.clone(),
        beta_handle: beta_handle.clone(),
        disabled_handle: disabled_handle.clone(),
        duplicate_handle: duplicate_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("listbox should publish a final accessibility tree");
    let beta_node = node_with_label(&initial, "Beta");
    let duplicate_a = node_with_label(&initial, "Duplicate A");
    let duplicate_b = node_with_label(&initial, "Duplicate B");
    assert_ne!(duplicate_a, duplicate_b);
    for duplicate in [duplicate_a, duplicate_b] {
        let node = initial
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == duplicate).then_some(node))
            .expect("duplicate option should remain in the final tree");
        assert!(node.is_disabled());
        assert!(!node.supports_action(accesskit::Action::Click));
    }
    for prefix in [
        "listbox:semantic-listbox:duplicate-option:4:shared",
        "listbox:semantic-listbox:duplicate-option:5:shared",
    ] {
        let selector = sole_debug_selector_with_prefix(cx, prefix);
        assert!(cx.debug_bounds(&selector).is_some());
    }

    let beta_bounds = cx
        .debug_bounds("listbox:semantic-listbox:option:beta")
        .expect("Beta should expose a stable selector");
    cx.simulate_click(beta_bounds.center(), Modifiers::none());
    assert_eq!(fallback.borrow().as_slice(), &["beta"]);
    assert!(cx.debug_selector_is_focused("listbox:semantic-listbox:option:beta"));

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert_eq!(fallback.borrow().len(), 1, "Enter activates on key-up");
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert_eq!(fallback.borrow().len(), 2);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(fallback.borrow().len(), 2, "Space activates on key-up");
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(fallback.borrow().len(), 3);

    let repeat_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), true));
    let repeat_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(repeat_down.propagated());
    assert!(repeat_up.propagated());
    assert_eq!(fallback.borrow().len(), 3);

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    cx.simulate_event(key_down("enter", modified, false));
    cx.simulate_event(key_up("enter", modified));
    assert_eq!(fallback.borrow().len(), 3);

    let modified_down = cx.simulate_event_with_dispatch_snapshot(key_down("down", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("down", modified));
    assert!(modified_down.propagated());
    assert!(!modified_down.default_prevented());
    assert!(modified_up.propagated());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("listbox:semantic-listbox:option:beta"));

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, beta_node,)));
    cx.update(|window, cx| {
        assert_eq!(
            beta_handle.request(window, cx),
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
        fallback.borrow().as_slice(),
        &["beta", "beta", "beta", "beta", "beta"]
    );

    let gamma_bounds = cx
        .debug_bounds("listbox:semantic-listbox:option:gamma")
        .expect("Gamma should expose a stable selector");
    cx.simulate_click(gamma_bounds.center(), Modifiers::none());
    assert_eq!(item.borrow().as_slice(), &["gamma"]);
    assert_eq!(
        fallback.borrow().len(),
        5,
        "the item handler replaces the listbox fallback"
    );
}

#[open_gpui::test]
fn controlled_listbox_emits_intent_without_mutating_owner_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        selections: Rc<RefCell<Vec<ListboxSelection>>>,
        beta_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            Listbox::new("controlled-listbox", "Controlled listbox")
                .selected(Some("alpha".to_owned()))
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::new("beta", "Beta"))
                .activation_handle("beta", &self.beta_handle)
                .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let beta_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        selections: selections.clone(),
        beta_handle: beta_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());

    cx.update(|window, cx| {
        assert_eq!(
            beta_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });

    assert_eq!(selections.borrow().len(), 1);
    assert_eq!(selections.borrow()[0].value(), "beta");
    let update = cx
        .latest_accessibility_tree_update()
        .expect("controlled listbox should publish its retained owner state");
    assert_eq!(selected_state(&update, "Alpha"), Some(true));
    assert_eq!(selected_state(&update, "Beta"), Some(false));
}

#[open_gpui::test]
fn disabled_listbox_blocks_every_activation_entry_and_preserves_semantics(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<String>>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Listbox::new("disabled-semantic-listbox", "Disabled semantic listbox")
                .disabled(true)
                .default_selected("alpha")
                .option(ListboxOption::new("alpha", "Disabled Alpha"))
                .activation_handle("alpha", &self.handle)
                .on_select(move |selection, _, _| {
                    activations.borrow_mut().push(selection.value().to_owned())
                })
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("disabled listbox should publish a final accessibility tree");
    let node_id = node_with_label(&update, "Disabled Alpha");
    let node = update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == node_id).then_some(node))
        .expect("disabled option should remain in the final tree");
    assert!(node.is_disabled());
    assert!(!node.supports_action(accesskit::Action::Click));

    let bounds = cx
        .debug_bounds("listbox:disabled-semantic-listbox:option:alpha")
        .expect("disabled option should remain rendered");
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, node_id,)));
    cx.update(|window, cx| {
        assert_eq!(handle.request(window, cx), ActivationRequestResult::Blocked);
    });
    assert!(activations.borrow().is_empty());
}

#[open_gpui::test]
fn listbox_activation_runtime_is_isolated_by_component_instance(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<String>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            div()
                .child(
                    Listbox::new("enabled-instance-listbox", "Enabled instance")
                        .default_selected("shared")
                        .option(ListboxOption::new("shared", "Enabled shared"))
                        .on_select(move |selection, _, _| {
                            activations.borrow_mut().push(selection.value().to_owned())
                        }),
                )
                .child(
                    Listbox::new("disabled-instance-listbox", "Disabled instance")
                        .option(ListboxOption::new("shared", "Disabled shared").disabled(true)),
                )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let enabled = cx
        .debug_bounds("listbox:enabled-instance-listbox:option:shared")
        .expect("enabled option should render");
    cx.simulate_click(enabled.center(), Modifiers::none());
    activations.borrow_mut().clear();

    let down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(down.propagation_stopped());
    assert!(activations.borrow().is_empty(), "Enter activates on key-up");

    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(up.propagation_stopped());
    assert_eq!(activations.borrow().as_slice(), &["shared"]);
}

#[open_gpui::test]
fn uncontrolled_listbox_retains_selection_across_disabled_projection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        disabled: bool,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Listbox::new("disable-cycle-listbox", "Disable cycle listbox")
                .disabled(self.disabled)
                .default_selected("alpha")
                .option(ListboxOption::new("alpha", "Persistent Alpha"))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| Probe { disabled: false });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("enabled listbox should publish");
    assert_eq!(selected_state(&initial, "Persistent Alpha"), Some(true));

    view.update(cx, |probe, cx| {
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled listbox should publish");
    assert_eq!(selected_state(&disabled, "Persistent Alpha"), Some(false));

    view.update(cx, |probe, cx| {
        probe.disabled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let reenabled = cx
        .latest_accessibility_tree_update()
        .expect("re-enabled listbox should publish");
    assert_eq!(selected_state(&reenabled, "Persistent Alpha"), Some(true));
}

#[open_gpui::test]
fn controlled_listbox_mirrors_owner_selection_while_disabled(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        disabled: bool,
        controlled: bool,
        selected_value: String,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let listbox =
                Listbox::new("disabled-controlled-listbox", "Disabled controlled listbox")
                    .disabled(self.disabled)
                    .option(ListboxOption::new("alpha", "Controlled Alpha"))
                    .option(ListboxOption::new("beta", "Controlled Beta"));
            if self.controlled {
                listbox.selected(Some(self.selected_value.clone()))
            } else {
                listbox
            }
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| Probe {
        disabled: false,
        controlled: true,
        selected_value: "alpha".to_owned(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("controlled listbox should publish");
    assert_eq!(selected_state(&initial, "Controlled Alpha"), Some(true));
    assert_eq!(selected_state(&initial, "Controlled Beta"), Some(false));

    view.update(cx, |probe, cx| {
        probe.disabled = true;
        probe.selected_value = "beta".to_owned();
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled controlled listbox should publish");
    assert_eq!(selected_state(&disabled, "Controlled Alpha"), Some(false));
    assert_eq!(selected_state(&disabled, "Controlled Beta"), Some(false));

    view.update(cx, |probe, cx| {
        probe.controlled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    view.update(cx, |probe, cx| {
        probe.disabled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let released = cx
        .latest_accessibility_tree_update()
        .expect("re-enabled uncontrolled listbox should publish");
    assert_eq!(selected_state(&released, "Controlled Alpha"), Some(false));
    assert_eq!(
        selected_state(&released, "Controlled Beta"),
        Some(true),
        "releasing ownership must retain the last caller-committed selection"
    );
}

#[open_gpui::test]
fn controlled_listbox_retains_unresolved_owner_selection_on_release(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Copy)]
    enum OptionShape {
        Missing,
        Disabled,
        Duplicate,
        Unique,
    }

    struct Probe {
        controlled: bool,
        shape: OptionShape,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let listbox = Listbox::new("unresolved-controlled-listbox", "Unresolved listbox")
                .option(ListboxOption::new("alpha", "Unresolved Alpha"));
            let listbox = match self.shape {
                OptionShape::Missing => listbox,
                OptionShape::Disabled => {
                    listbox.option(ListboxOption::new("beta", "Disabled Beta").disabled(true))
                }
                OptionShape::Duplicate => listbox
                    .option(ListboxOption::new("beta", "Duplicate Beta A"))
                    .option(ListboxOption::new("beta", "Duplicate Beta B")),
                OptionShape::Unique => listbox.option(ListboxOption::new("beta", "Resolved Beta")),
            };
            if self.controlled {
                listbox.selected(Some("beta".to_owned()))
            } else {
                listbox
            }
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| Probe {
        controlled: true,
        shape: OptionShape::Missing,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());

    view.update(cx, |probe, cx| {
        probe.shape = OptionShape::Disabled;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled owner target should publish");
    assert_eq!(selected_state(&disabled, "Disabled Beta"), Some(false));

    view.update(cx, |probe, cx| {
        probe.shape = OptionShape::Duplicate;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let duplicate = cx
        .latest_accessibility_tree_update()
        .expect("duplicate owner target should publish");
    assert_eq!(selected_state(&duplicate, "Duplicate Beta A"), Some(false));
    assert_eq!(selected_state(&duplicate, "Duplicate Beta B"), Some(false));

    view.update(cx, |probe, cx| {
        probe.controlled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    view.update(cx, |probe, cx| {
        probe.shape = OptionShape::Unique;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let resolved = cx
        .latest_accessibility_tree_update()
        .expect("resolved owner target should publish");
    assert_eq!(
        selected_state(&resolved, "Resolved Beta"),
        Some(true),
        "runtime must retain the raw owner commit while projection is unresolved"
    );
}
