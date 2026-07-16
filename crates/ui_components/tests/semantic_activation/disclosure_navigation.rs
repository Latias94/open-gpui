use super::*;

use open_gpui_ui_components::{
    Accordion, AccordionItem, Breadcrumb, BreadcrumbItemDescriptor, Collapsible,
};

fn accessibility_node<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    let id = node_with_label(update, label);
    let node = update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then_some(node))
        .expect("labelled accessibility node should exist");
    (id, node)
}

#[open_gpui::test]
fn accordion_item_routes_every_activation_source_to_one_controlled_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    type ChangeRecord = (String, Vec<String>);

    struct Probe {
        changes: Rc<RefCell<Vec<ChangeRecord>>>,
        billing_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            Accordion::new("semantic-accordion")
                .open_values(Vec::<String>::new())
                .item(AccordionItem::new(
                    "billing",
                    "Billing settings",
                    "Invoice settings",
                ))
                .item(AccordionItem::new("disabled", "Managed settings", "Managed").disabled(true))
                .activation_handle("billing", &self.billing_handle)
                .activation_handle("disabled", &self.disabled_handle)
                .on_open_change(move |change, _, _| {
                    changes.borrow_mut().push((
                        change.item().value().to_owned(),
                        change.open_values().to_vec(),
                    ));
                })
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let billing_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        billing_handle: billing_handle.clone(),
        disabled_handle: disabled_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("accordion should publish a final accessibility tree");
    let (billing_node, billing) = accessibility_node(&update, "Billing settings");
    let (_, disabled) = accessibility_node(&update, "Managed settings");
    assert!(billing.supports_action(accesskit::Action::Click));
    assert_eq!(billing.is_expanded(), Some(false));
    assert!(!disabled.supports_action(accesskit::Action::Click));

    let bounds = cx
        .debug_bounds("accordion:semantic-accordion:item:billing:trigger")
        .expect("accordion trigger should expose a stable selector");
    cx.simulate_click(bounds.center(), Modifiers::none());

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, billing_node,))
    );
    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagation_stopped());
    assert!(enter_up.propagation_stopped());

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, billing_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            billing_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 5);
    assert!(
        changes.iter().all(|(item, open_values)| {
            item == "billing" && open_values.as_slice() == ["billing"]
        })
    );
    drop(changes);

    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("controlled accordion redraw should preserve owner state");
    let (_, billing) = accessibility_node(&update, "Billing settings");
    assert_eq!(billing.is_expanded(), Some(false));
}

#[open_gpui::test]
fn collapsible_emits_controlled_open_intent_and_blocks_disabled_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        changes: Rc<RefCell<Vec<bool>>>,
        disabled_changes: Rc<RefCell<Vec<bool>>>,
        handle: ActivationHandle,
        disabled_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let disabled_changes = self.disabled_changes.clone();
            div()
                .flex()
                .flex_col()
                .child(
                    Collapsible::new("semantic-collapsible", "Release notes")
                        .open(false)
                        .content("Controlled content")
                        .activation_handle(&self.handle)
                        .on_open_change(move |open, _, _| changes.borrow_mut().push(open)),
                )
                .child(
                    Collapsible::new("disabled-collapsible", "Managed disclosure")
                        .disabled(true)
                        .activation_handle(&self.disabled_handle)
                        .on_open_change(move |open, _, _| {
                            disabled_changes.borrow_mut().push(open);
                        }),
                )
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let disabled_changes = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        changes: changes.clone(),
        disabled_changes: disabled_changes.clone(),
        handle: handle.clone(),
        disabled_handle: disabled_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("collapsible should publish a final accessibility tree");
    let (trigger_node, trigger) = accessibility_node(&update, "Release notes");
    let (_, disabled_trigger) = accessibility_node(&update, "Managed disclosure");
    assert!(trigger.supports_action(accesskit::Action::Click));
    assert_eq!(trigger.is_expanded(), Some(false));
    assert!(!disabled_trigger.supports_action(accesskit::Action::Click));

    let bounds = cx
        .debug_bounds("collapsible:semantic-collapsible:trigger")
        .expect("collapsible trigger should expose a stable selector");
    let disabled_bounds = cx
        .debug_bounds("collapsible:disabled-collapsible:trigger")
        .expect("disabled collapsible should expose a stable selector");
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.simulate_click(disabled_bounds.center(), Modifiers::none());

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, trigger_node,))
    );
    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_down.default_prevented());
    assert!(space_up.default_prevented());

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, trigger_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    assert_eq!(changes.borrow().as_slice(), &[true, true, true, true]);
    assert!(disabled_changes.borrow().is_empty());

    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("controlled redraw should publish the current owner state");
    let (_, trigger) = accessibility_node(&update, "Release notes");
    assert_eq!(trigger.is_expanded(), Some(false));
}

#[open_gpui::test]
fn breadcrumb_accepts_enter_rejects_space_and_preserves_domain_addressing(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationRecord = (String, Option<String>, ActivationSource);

    struct Probe {
        activations: Rc<RefCell<Vec<ActivationRecord>>>,
        home_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
        current_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Breadcrumb::new("semantic-breadcrumb", "Project path")
                .item(BreadcrumbItemDescriptor::new("home", "Home").href("/"))
                .item(
                    BreadcrumbItemDescriptor::new("managed", "Managed")
                        .href("/managed")
                        .disabled(true),
                )
                .item(BreadcrumbItemDescriptor::new("current", "Current"))
                .activation_handle("home", &self.home_handle)
                .activation_handle("managed", &self.disabled_handle)
                .activation_handle("current", &self.current_handle)
                .on_activate(move |payload, activation, _, _| {
                    activations.borrow_mut().push((
                        payload.value().to_owned(),
                        payload.href().map(str::to_owned),
                        activation.source(),
                    ));
                })
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let home_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let current_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        home_handle: home_handle.clone(),
        disabled_handle: disabled_handle.clone(),
        current_handle: current_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("breadcrumb should publish a final accessibility tree");
    let (home_node, home) = accessibility_node(&update, "Home");
    let (_, managed) = accessibility_node(&update, "Managed");
    let (_, current) = accessibility_node(&update, "Current");
    assert!(home.supports_action(accesskit::Action::Click));
    assert!(!managed.supports_action(accesskit::Action::Click));
    assert!(!current.supports_action(accesskit::Action::Click));

    let bounds = cx
        .debug_bounds("breadcrumb:semantic-breadcrumb:item:home")
        .expect("breadcrumb item should expose a stable selector");
    cx.simulate_click(bounds.center(), Modifiers::none());

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, home_node,)));
    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagation_stopped());
    assert!(enter_up.propagation_stopped());

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_down.propagated());
    assert!(!space_down.default_prevented());
    assert!(space_up.propagated());
    assert!(!space_up.default_prevented());

    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, home_node,)));
    cx.update(|window, cx| {
        assert_eq!(
            home_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            current_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    assert_eq!(
        activations.borrow().as_slice(),
        &[
            (
                "home".to_owned(),
                Some("/".to_owned()),
                ActivationSource::Pointer,
            ),
            (
                "home".to_owned(),
                Some("/".to_owned()),
                ActivationSource::Keyboard(ActivationKey::Enter),
            ),
            (
                "home".to_owned(),
                Some("/".to_owned()),
                ActivationSource::Accessibility,
            ),
            (
                "home".to_owned(),
                Some("/".to_owned()),
                ActivationSource::Programmatic,
            ),
        ]
    );
}
