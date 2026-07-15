use super::*;

use open_gpui_ui_components::{Tag, Toast, ToastDismissReason, ToastStack};

fn accessibility_node_with_label<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> &'a accesskit::Node {
    let id = node_with_label(update, label);
    update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then_some(node))
        .expect("labelled accessibility node should exist")
}

#[open_gpui::test]
fn tag_remove_preserves_payload_and_routes_each_activation_source_once(
    cx: &mut open_gpui::TestAppContext,
) {
    type TagRecord = (String, String, ActivationSource);

    struct Probe {
        records: Rc<RefCell<Vec<TagRecord>>>,
        disabled_records: Rc<RefCell<Vec<TagRecord>>>,
        handle: ActivationHandle,
        disabled_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let records = self.records.clone();
            let disabled_records = self.disabled_records.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Tag::new("semantic-tag", "ready", "Ready")
                        .on_remove(move |payload, activation, _, _| {
                            records.borrow_mut().push((
                                payload.value().to_owned(),
                                payload.label().to_owned(),
                                activation.source(),
                            ));
                        })
                        .activation_handle(&self.handle),
                )
                .child(
                    Tag::new("disabled-semantic-tag", "locked", "Locked")
                        .disabled(true)
                        .on_remove(move |payload, activation, _, _| {
                            disabled_records.borrow_mut().push((
                                payload.value().to_owned(),
                                payload.label().to_owned(),
                                activation.source(),
                            ));
                        })
                        .activation_handle(&self.disabled_handle),
                )
        }
    }

    let records = Rc::new(RefCell::new(Vec::new()));
    let disabled_records = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        records: records.clone(),
        disabled_records: disabled_records.clone(),
        handle: handle.clone(),
        disabled_handle: disabled_handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let remove_bounds = cx
        .debug_bounds("tag:semantic-tag:remove")
        .expect("Tag remove should expose a stable selector");
    cx.simulate_click(remove_bounds.center(), Modifiers::none());
    assert_eq!(records.borrow().len(), 1);

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Tag remove controls should publish an accessibility tree");
    let remove_node = node_with_label(&update, "Remove Ready");
    let disabled_remove_node = node_with_label(&update, "Remove Locked");
    assert!(
        accessibility_node_with_label(&update, "Remove Ready")
            .supports_action(accesskit::Action::Click)
    );
    assert!(
        !accessibility_node_with_label(&update, "Remove Locked")
            .supports_action(accesskit::Action::Click)
    );
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, remove_node,))
    );

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert_eq!(
        records.borrow().len(),
        1,
        "Enter key-down must not activate"
    );
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert_eq!(records.borrow().len(), 2);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(
        records.borrow().len(),
        2,
        "Space key-down must not activate"
    );
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(records.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, remove_node,))
    );
    assert_eq!(records.borrow().len(), 4);

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

    assert_eq!(
        records.borrow().as_slice(),
        &[
            (
                "ready".to_owned(),
                "Ready".to_owned(),
                ActivationSource::Pointer,
            ),
            (
                "ready".to_owned(),
                "Ready".to_owned(),
                ActivationSource::Keyboard(ActivationKey::Enter),
            ),
            (
                "ready".to_owned(),
                "Ready".to_owned(),
                ActivationSource::Keyboard(ActivationKey::Space),
            ),
            (
                "ready".to_owned(),
                "Ready".to_owned(),
                ActivationSource::Accessibility,
            ),
            (
                "ready".to_owned(),
                "Ready".to_owned(),
                ActivationSource::Programmatic,
            ),
        ]
    );

    let disabled_bounds = cx
        .debug_bounds("tag:disabled-semantic-tag:remove")
        .expect("disabled Tag remove should keep its stable selector");
    cx.simulate_click(disabled_bounds.center(), Modifiers::none());
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        disabled_remove_node,
    )));
    assert!(disabled_records.borrow().is_empty());
}

#[open_gpui::test]
fn toast_action_is_domain_addressed_and_routes_each_activation_source_once(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActionRecord = (String, String, ActivationSource);

    struct Probe {
        records: Rc<RefCell<Vec<ActionRecord>>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let records = self.records.clone();

            ToastStack::new("semantic-action-stack", "Notifications")
                .toast(
                    Toast::new("other", "Other")
                        .action("Archive")
                        .dismissible(false),
                )
                .toast(
                    Toast::new("saved", "Saved")
                        .action("Undo")
                        .dismissible(false),
                )
                .on_action(move |payload, activation, _, _| {
                    records.borrow_mut().push((
                        payload.id().to_owned(),
                        payload.label().to_owned(),
                        activation.source(),
                    ));
                })
                .action_activation_handle("saved", &self.handle)
        }
    }

    let records = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        records: records.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let action_bounds = cx
        .debug_bounds("toast-stack:semantic-action-stack:toast:saved:action")
        .expect("Toast action should expose a stable domain selector");
    cx.simulate_click(action_bounds.center(), Modifiers::none());
    assert_eq!(records.borrow().len(), 1);

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Toast actions should publish an accessibility tree");
    let action_node = node_with_label(&update, "Undo");
    assert!(
        accessibility_node_with_label(&update, "Undo").supports_action(accesskit::Action::Click)
    );
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, action_node,))
    );

    cx.simulate_event(key_down("enter", Modifiers::none(), false));
    assert_eq!(records.borrow().len(), 1);
    cx.simulate_event(key_up("enter", Modifiers::none()));
    assert_eq!(records.borrow().len(), 2);
    cx.simulate_event(key_down("space", Modifiers::none(), false));
    assert_eq!(records.borrow().len(), 2);
    cx.simulate_event(key_up("space", Modifiers::none()));
    assert_eq!(records.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, action_node,))
    );
    assert_eq!(records.borrow().len(), 4);
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });

    let expected_sources = [
        ActivationSource::Pointer,
        ActivationSource::Keyboard(ActivationKey::Enter),
        ActivationSource::Keyboard(ActivationKey::Space),
        ActivationSource::Accessibility,
        ActivationSource::Programmatic,
    ];
    assert_eq!(records.borrow().len(), expected_sources.len());
    for (record, expected_source) in records.borrow().iter().zip(expected_sources) {
        assert_eq!(record.0, "saved");
        assert_eq!(record.1, "Undo");
        assert_eq!(record.2, expected_source);
    }
}

#[open_gpui::test]
fn toast_dismiss_is_domain_addressed_and_routes_each_activation_source_once(
    cx: &mut open_gpui::TestAppContext,
) {
    type DismissRecord = (String, ToastDismissReason, ActivationSource);

    struct Probe {
        records: Rc<RefCell<Vec<DismissRecord>>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let records = self.records.clone();

            ToastStack::new("semantic-dismiss-stack", "Notifications")
                .toast(Toast::new("other", "Other").dismissible(false))
                .toast(Toast::new("saved", "Saved"))
                .on_dismiss(move |payload, activation, _, _| {
                    records.borrow_mut().push((
                        payload.id().to_owned(),
                        payload.reason(),
                        activation.source(),
                    ));
                })
                .dismiss_activation_handle("saved", &self.handle)
        }
    }

    let records = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        records: records.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let dismiss_bounds = cx
        .debug_bounds("toast-stack:semantic-dismiss-stack:toast:saved:dismiss")
        .expect("Toast dismiss should expose a stable domain selector");
    cx.simulate_click(dismiss_bounds.center(), Modifiers::none());
    assert_eq!(records.borrow().len(), 1);

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Toast dismiss controls should publish an accessibility tree");
    let dismiss_node = node_with_label(&update, "Dismiss notification");
    assert!(
        accessibility_node_with_label(&update, "Dismiss notification")
            .supports_action(accesskit::Action::Click)
    );
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, dismiss_node,))
    );

    cx.simulate_event(key_down("enter", Modifiers::none(), false));
    assert_eq!(records.borrow().len(), 1);
    cx.simulate_event(key_up("enter", Modifiers::none()));
    assert_eq!(records.borrow().len(), 2);
    cx.simulate_event(key_down("space", Modifiers::none(), false));
    assert_eq!(records.borrow().len(), 2);
    cx.simulate_event(key_up("space", Modifiers::none()));
    assert_eq!(records.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, dismiss_node,))
    );
    assert_eq!(records.borrow().len(), 4);
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });

    let expected_sources = [
        ActivationSource::Pointer,
        ActivationSource::Keyboard(ActivationKey::Enter),
        ActivationSource::Keyboard(ActivationKey::Space),
        ActivationSource::Accessibility,
        ActivationSource::Programmatic,
    ];
    assert_eq!(records.borrow().len(), expected_sources.len());
    for (record, expected_source) in records.borrow().iter().zip(expected_sources) {
        assert_eq!(record.0, "saved");
        assert_eq!(record.1, ToastDismissReason::Manual);
        assert_eq!(record.2, expected_source);
    }
}

#[open_gpui::test]
fn domain_affordances_without_handlers_do_not_declare_click(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        tag_handle: ActivationHandle,
        action_handle: ActivationHandle,
        dismiss_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Tag::new("passive-tag", "passive", "Passive tag")
                        .removable(true)
                        .activation_handle(&self.tag_handle),
                )
                .child(
                    ToastStack::new("passive-toast-stack", "Passive notifications")
                        .toast(Toast::new("passive", "Passive toast").action("Inspect"))
                        .action_activation_handle("passive", &self.action_handle)
                        .dismiss_activation_handle("passive", &self.dismiss_handle),
                )
        }
    }

    let tag_handle = ActivationHandle::new();
    let action_handle = ActivationHandle::new();
    let dismiss_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        tag_handle: tag_handle.clone(),
        action_handle: action_handle.clone(),
        dismiss_handle: dismiss_handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("passive domain controls should publish an accessibility tree");
    assert!(
        !accessibility_node_with_label(&update, "Passive tag")
            .supports_action(accesskit::Action::Click)
    );
    assert!(
        !accessibility_node_with_label(&update, "Inspect")
            .supports_action(accesskit::Action::Click)
    );
    assert!(
        !accessibility_node_with_label(&update, "Dismiss notification")
            .supports_action(accesskit::Action::Click)
    );

    cx.update(|window, cx| {
        assert_eq!(
            tag_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
        assert_eq!(
            action_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
        assert_eq!(
            dismiss_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
}
