use super::*;

use open_gpui_ui_components::{Tabs, TabsActivationMode, TabsItem};

fn selected_state(update: &accesskit::TreeUpdate, label: &str) -> Option<bool> {
    let id = node_with_label(update, label);
    update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then(|| node.is_selected()))
        .flatten()
}

fn assert_no_selected_panel_relations(update: &accesskit::TreeUpdate, tab_labels: &[&str]) {
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.role() != accesskit::Role::TabPanel),
        "tabs without a selected value must not publish an unnamed tab panel"
    );

    for label in tab_labels {
        let tab_id = node_with_label(update, label);
        let tab = update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == tab_id).then_some(node))
            .expect("labelled tab should exist in the final accessibility tree");
        assert!(
            tab.controls().is_empty(),
            "unselected tab `{label}` must not control a missing tab panel"
        );
    }
}

fn controlled_tabs_case(
    id: &'static str,
    selected: Option<String>,
    activation_handle: &ActivationHandle,
    intents: Rc<RefCell<Vec<String>>>,
) -> Tabs {
    let overview_label = format!("{id} Overview");
    let details_label = format!("{id} Details");
    let overview_panel_selector = format!("controlled-tabs-panel:{id}:overview");
    let details_panel_selector = format!("controlled-tabs-panel:{id}:details");
    let intent_owner = id.to_owned();

    Tabs::new(id)
        .selected(selected)
        .item(TabsItem::new(
            "overview",
            overview_label,
            div().debug_selector(move || overview_panel_selector.clone()),
        ))
        .item(TabsItem::new(
            "details",
            details_label,
            div().debug_selector(move || details_panel_selector.clone()),
        ))
        .activation_handle("details", activation_handle)
        .on_selection_change(move |selection, _, _| {
            intents
                .borrow_mut()
                .push(format!("{intent_owner}:{}", selection.value()));
        })
}

fn assert_automatic_tabs_state(
    cx: &mut open_gpui::VisualTestContext,
    selected_value: &str,
    selected_label: &str,
    selected_panel: &str,
    hidden_panels: &[&str],
) {
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused(&format!(
        "tabs:automatic-semantic-tabs:trigger:{selected_value}"
    )));
    assert!(cx.debug_bounds(selected_panel).is_some());
    for panel in hidden_panels {
        assert!(cx.debug_bounds(panel).is_none());
    }

    let update = cx
        .latest_accessibility_tree_update()
        .expect("automatic tabs should publish a final accessibility tree");
    assert_eq!(selected_state(&update, selected_label), Some(true));
}

#[open_gpui::test]
fn tabs_route_every_activation_source_to_controlled_selection_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        selections: Rc<RefCell<Vec<String>>>,
        details_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            Tabs::new("controlled-semantic-tabs")
                .activation_mode(TabsActivationMode::Manual)
                .selected(Some("overview".to_owned()))
                .item(TabsItem::new(
                    "overview",
                    "Overview",
                    div().child("Overview panel"),
                ))
                .item(TabsItem::new(
                    "details",
                    "Details",
                    div().child("Details panel"),
                ))
                .item(
                    TabsItem::new("managed", "Managed", div().child("Managed panel"))
                        .disabled(true),
                )
                .activation_handle("details", &self.details_handle)
                .activation_handle("managed", &self.disabled_handle)
                .on_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                })
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let details_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        selections: selections.clone(),
        details_handle: details_handle.clone(),
        disabled_handle: disabled_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("tabs should publish a final accessibility tree");
    let details_node = node_with_label(&initial, "Details");
    let managed_node = node_with_label(&initial, "Managed");
    assert_eq!(selected_state(&initial, "Overview"), Some(true));
    assert_eq!(selected_state(&initial, "Details"), Some(false));
    let managed = initial
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == managed_node).then_some(node))
        .expect("managed tab should exist");
    assert!(!managed.supports_action(accesskit::Action::Click));

    let details_bounds = cx
        .debug_bounds("tabs:controlled-semantic-tabs:trigger:details")
        .expect("details tab should expose a stable selector");
    cx.simulate_click(details_bounds.center(), Modifiers::none());
    assert_eq!(selections.borrow().as_slice(), &["details"]);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, details_node,))
    );
    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert_eq!(
        selections.borrow().len(),
        1,
        "Enter must activate on key-up rather than key-down"
    );
    let enter_repeat =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), true));
    assert!(enter_repeat.propagation_stopped());
    assert_eq!(selections.borrow().len(), 1);
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert_eq!(selections.borrow().as_slice(), &["details", "details"]);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(selections.borrow().len(), 2);
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(
        selections.borrow().as_slice(),
        &["details", "details", "details"]
    );

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, details_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            details_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(
        selections.borrow().as_slice(),
        &["details", "details", "details", "details", "details"]
    );

    cx.update(|window, cx| window.draw(cx).clear());
    let controlled = cx
        .latest_accessibility_tree_update()
        .expect("controlled tabs redraw should preserve caller state");
    assert_eq!(selected_state(&controlled, "Overview"), Some(true));
    assert_eq!(selected_state(&controlled, "Details"), Some(false));
}

#[open_gpui::test]
fn controlled_tabs_preserve_empty_and_unknown_owner_state_after_rejected_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        intents: Rc<RefCell<Vec<String>>>,
        empty_details: ActivationHandle,
        unknown_details: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(controlled_tabs_case(
                    "controlled-empty",
                    None,
                    &self.empty_details,
                    self.intents.clone(),
                ))
                .child(controlled_tabs_case(
                    "controlled-unknown",
                    Some("missing".to_owned()),
                    &self.unknown_details,
                    self.intents.clone(),
                ))
        }
    }

    let intents = Rc::new(RefCell::new(Vec::new()));
    let empty_details = ActivationHandle::new();
    let unknown_details = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        intents: intents.clone(),
        empty_details: empty_details.clone(),
        unknown_details: unknown_details.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("controlled tabs should publish a final accessibility tree");
    for label in [
        "controlled-empty Overview",
        "controlled-empty Details",
        "controlled-unknown Overview",
        "controlled-unknown Details",
    ] {
        assert_eq!(selected_state(&initial, label), Some(false));
    }
    assert_no_selected_panel_relations(
        &initial,
        &[
            "controlled-empty Overview",
            "controlled-empty Details",
            "controlled-unknown Overview",
            "controlled-unknown Details",
        ],
    );
    for panel in [
        "controlled-tabs-panel:controlled-empty:overview",
        "controlled-tabs-panel:controlled-empty:details",
        "controlled-tabs-panel:controlled-unknown:overview",
        "controlled-tabs-panel:controlled-unknown:details",
    ] {
        assert!(cx.debug_bounds(panel).is_none());
    }

    cx.update(|window, cx| {
        assert_eq!(
            empty_details.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            unknown_details.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(
        intents.borrow().as_slice(),
        &["controlled-empty:details", "controlled-unknown:details"]
    );

    cx.update(|window, cx| window.draw(cx).clear());
    let rejected = cx
        .latest_accessibility_tree_update()
        .expect("controlled tabs redraw should preserve rejected owner state");
    for label in [
        "controlled-empty Overview",
        "controlled-empty Details",
        "controlled-unknown Overview",
        "controlled-unknown Details",
    ] {
        assert_eq!(selected_state(&rejected, label), Some(false));
    }
    assert_no_selected_panel_relations(
        &rejected,
        &[
            "controlled-empty Overview",
            "controlled-empty Details",
            "controlled-unknown Overview",
            "controlled-unknown Details",
        ],
    );
    for panel in [
        "controlled-tabs-panel:controlled-empty:overview",
        "controlled-tabs-panel:controlled-empty:details",
        "controlled-tabs-panel:controlled-unknown:overview",
        "controlled-tabs-panel:controlled-unknown:details",
    ] {
        assert!(cx.debug_bounds(panel).is_none());
    }
}

#[open_gpui::test]
fn uncontrolled_tabs_commit_before_callback_reentry(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        selections: Rc<RefCell<Vec<String>>>,
        reentered: Rc<Cell<bool>>,
        details_handle: ActivationHandle,
        history_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let reentered = self.reentered.clone();
            let details_handle = self.details_handle.clone();
            let history_handle = self.history_handle.clone();
            Tabs::new("uncontrolled-semantic-tabs")
                .activation_mode(TabsActivationMode::Manual)
                .default_selected("overview")
                .item(TabsItem::new(
                    "overview",
                    "Overview",
                    div().child("Overview panel"),
                ))
                .item(TabsItem::new(
                    "details",
                    "Details",
                    div().child("Details panel"),
                ))
                .item(TabsItem::new(
                    "history",
                    "History",
                    div().child("History panel"),
                ))
                .activation_handle("details", &self.details_handle)
                .activation_handle("history", &self.history_handle)
                .on_selection_change(move |selection, window, cx| {
                    selections.borrow_mut().push(selection.value().to_owned());
                    if !reentered.replace(true) {
                        assert_eq!(
                            details_handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                        assert_eq!(
                            selections.borrow().as_slice(),
                            &["details"],
                            "same-value reentry must observe the committed selection"
                        );
                        assert_eq!(
                            history_handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                    }
                })
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let reentered = Rc::new(Cell::new(false));
    let details_handle = ActivationHandle::new();
    let history_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        selections: selections.clone(),
        reentered: reentered.clone(),
        details_handle: details_handle.clone(),
        history_handle: history_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("uncontrolled tabs should publish a final accessibility tree");
    let history_node = node_with_label(&initial, "History");

    cx.update(|window, cx| {
        assert_eq!(
            details_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(selections.borrow().as_slice(), &["details", "history"]);
    assert!(reentered.get());
    assert!(cx.debug_selector_is_focused("tabs:uncontrolled-semantic-tabs:trigger:history"));

    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("uncontrolled reentrant commit should reach the final tree");
    assert_eq!(update.focus, history_node);
    assert_eq!(selected_state(&update, "Overview"), Some(false));
    assert_eq!(selected_state(&update, "Details"), Some(false));
    assert_eq!(selected_state(&update, "History"), Some(true));
}

#[open_gpui::test]
fn automatic_tabs_keydown_keeps_selection_focus_panel_and_callback_in_sync(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        selections: Rc<RefCell<Vec<String>>>,
        prevent_navigation: Rc<Cell<bool>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let prevent_navigation = self.prevent_navigation.clone();

            div()
                .capture_key_down(move |event, window, _| {
                    if prevent_navigation.get() && event.keystroke.key == "right" {
                        window.prevent_default();
                    }
                })
                .child(
                    Tabs::new("automatic-semantic-tabs")
                        .default_selected("overview")
                        .item(TabsItem::new(
                            "overview",
                            "Automatic Overview",
                            div().debug_selector(|| "automatic-tabs-panel:overview".to_owned()),
                        ))
                        .item(
                            TabsItem::new(
                                "billing",
                                "Automatic Billing",
                                div().debug_selector(|| "automatic-tabs-panel:billing".to_owned()),
                            )
                            .disabled(true),
                        )
                        .item(TabsItem::new(
                            "details",
                            "Automatic Details",
                            div().debug_selector(|| "automatic-tabs-panel:details".to_owned()),
                        ))
                        .item(TabsItem::new(
                            "history",
                            "Automatic History",
                            div().debug_selector(|| "automatic-tabs-panel:history".to_owned()),
                        ))
                        .on_selection_change(move |selection, _, _| {
                            selections.borrow_mut().push(selection.value().to_owned());
                        }),
                )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let prevent_navigation = Rc::new(Cell::new(false));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        selections: selections.clone(),
        prevent_navigation: prevent_navigation.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("automatic tabs should publish a final accessibility tree");
    let overview_node = node_with_label(&initial, "Automatic Overview");
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, overview_node,))
    );
    assert_automatic_tabs_state(
        cx,
        "overview",
        "Automatic Overview",
        "automatic-tabs-panel:overview",
        &[
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:details",
            "automatic-tabs-panel:history",
        ],
    );

    prevent_navigation.set(true);
    let prevented =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", Modifiers::none(), false));
    prevent_navigation.set(false);
    assert!(prevented.default_prevented());
    assert!(prevented.propagated());
    assert!(selections.borrow().is_empty());
    assert_automatic_tabs_state(
        cx,
        "overview",
        "Automatic Overview",
        "automatic-tabs-panel:overview",
        &[
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:details",
            "automatic-tabs-panel:history",
        ],
    );

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_right =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", modified, false));
    assert!(modified_right.propagated());
    assert!(!modified_right.default_prevented());
    assert!(selections.borrow().is_empty());

    let right =
        cx.simulate_event_with_dispatch_snapshot(key_down("right", Modifiers::none(), false));
    assert!(right.propagation_stopped());
    assert_eq!(selections.borrow().as_slice(), &["details"]);
    assert_automatic_tabs_state(
        cx,
        "details",
        "Automatic Details",
        "automatic-tabs-panel:details",
        &[
            "automatic-tabs-panel:overview",
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:history",
        ],
    );

    let end = cx.simulate_event_with_dispatch_snapshot(key_down("end", Modifiers::none(), false));
    assert!(end.propagation_stopped());
    assert_eq!(selections.borrow().as_slice(), &["details", "history"]);
    assert_automatic_tabs_state(
        cx,
        "history",
        "Automatic History",
        "automatic-tabs-panel:history",
        &[
            "automatic-tabs-panel:overview",
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:details",
        ],
    );

    let home = cx.simulate_event_with_dispatch_snapshot(key_down("home", Modifiers::none(), false));
    assert!(home.propagation_stopped());
    assert_eq!(
        selections.borrow().as_slice(),
        &["details", "history", "overview"]
    );
    assert_automatic_tabs_state(
        cx,
        "overview",
        "Automatic Overview",
        "automatic-tabs-panel:overview",
        &[
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:details",
            "automatic-tabs-panel:history",
        ],
    );

    let left = cx.simulate_event_with_dispatch_snapshot(key_down("left", Modifiers::none(), false));
    assert!(left.propagation_stopped());
    assert_eq!(
        selections.borrow().as_slice(),
        &["details", "history", "overview", "history"]
    );
    assert_automatic_tabs_state(
        cx,
        "history",
        "Automatic History",
        "automatic-tabs-panel:history",
        &[
            "automatic-tabs-panel:overview",
            "automatic-tabs-panel:billing",
            "automatic-tabs-panel:details",
        ],
    );
}
