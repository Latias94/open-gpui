use std::cell::RefCell;
use std::rc::Rc;

use open_gpui::{
    Context, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render,
    Styled, Window, accesskit, div,
};
use open_gpui_ui_components::{
    ActivationHandle, ActivationRequestResult, RadioGroup, RadioGroupState, RadioItem,
    RadioItemDescriptor, RadioItemState, RadioSelection, RadioSelectionAuthority,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens};

use super::a11y_support::node_with_label as a11y_node_with_label;

fn key_down(key: &str, modifiers: Modifiers, is_held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        },
        is_held,
        prefer_character_input: false,
    }
}

fn key_up(key: &str, modifiers: Modifiers) -> KeyUpEvent {
    KeyUpEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        },
    }
}

#[open_gpui::test]
fn radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<RadioSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                RadioGroup::new("runtime-radio")
                    .label("Runtime radio")
                    .orientation(Orientation::Horizontal)
                    .default_selected("personal")
                    .item(RadioItem::new("personal", "Personal"))
                    .item(RadioItem::new("team", "Team").disabled(true))
                    .item(RadioItem::new("enterprise", "Enterprise"))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("radio-group:runtime-radio").is_some(),
        "radio group root should expose a stable debug selector"
    );

    let disabled_team = cx
        .debug_bounds("radio-group:runtime-radio:item:team")
        .expect("disabled Team radio item should be rendered");
    cx.simulate_click(disabled_team.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled radio click should not emit a selection change"
    );

    let enterprise = cx
        .debug_bounds("radio-group:runtime-radio:item:enterprise")
        .expect("Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 2);
    assert_eq!(after_click[0].value(), "enterprise");
    assert_eq!(after_click[0].label(), "Enterprise");

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_left = selections.borrow().clone();
    assert_eq!(after_left.len(), 2);
    assert_eq!(after_left[1].index(), 0);
    assert_eq!(after_left[1].value(), "personal");
    assert_eq!(after_left[1].label(), "Personal");

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagated());
    assert!(enter_up.propagated());
    assert_eq!(
        selections.borrow().len(),
        2,
        "Enter is not a radio activation key"
    );

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(
        selections.borrow().len(),
        2,
        "Space key-down must not activate"
    );
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    let after_space = selections.borrow().clone();
    assert_eq!(
        after_space.len(),
        2,
        "Space on the already selected radio should not emit a duplicate selection change"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 3);
    assert_eq!(after_right[2].index(), 2);
    assert_eq!(after_right[2].value(), "enterprise");
    assert_eq!(after_right[2].label(), "Enterprise");

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("radio group should publish an accessibility tree");
    let (personal_node, personal) = a11y_node_with_label(&update, "Personal");
    assert!(personal.supports_action(accesskit::Action::Click));
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: personal_node,
        data: None,
    }));
    let after_accessibility = selections.borrow().clone();
    assert_eq!(after_accessibility.len(), 4);
    assert_eq!(after_accessibility[3].value(), "personal");
}

#[open_gpui::test]
fn radio_group_controlled_selection_emits_intent_without_committing_owner_state(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected: Rc<RefCell<String>>,
        observations: Rc<RefCell<Vec<(String, String)>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selected_value = self.selected.borrow().clone();
            let selected = self.selected.clone();
            let observations = self.observations.clone();

            RadioGroup::new("controlled-radio")
                .label("Controlled radio")
                .selected(Some(selected_value))
                .default_selected("enterprise")
                .item(RadioItem::new("personal", "Controlled personal"))
                .item(RadioItem::new("enterprise", "Controlled enterprise"))
                .on_selection_change(move |selection, _, _| {
                    observations
                        .borrow_mut()
                        .push((selection.value().to_owned(), selected.borrow().clone()));
                })
        }
    }

    let selected = Rc::new(RefCell::new("personal".to_owned()));
    let observations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selected: selected.clone(),
        observations: observations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let enterprise = cx
        .debug_bounds("radio-group:controlled-radio:item:enterprise")
        .expect("controlled Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(
        observations.borrow().as_slice(),
        &[("enterprise".to_owned(), "personal".to_owned())]
    );
    assert_eq!(selected.borrow().as_str(), "personal");

    cx.update(|window, cx| window.draw(cx).clear());
    let enterprise = cx
        .debug_bounds("radio-group:controlled-radio:item:enterprise")
        .expect("controlled Enterprise radio item should remain rendered");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(
        observations.borrow().as_slice(),
        &[
            ("enterprise".to_owned(), "personal".to_owned()),
            ("enterprise".to_owned(), "personal".to_owned()),
        ],
        "each gesture should emit intent against the live caller-owned selection"
    );

    *selected.borrow_mut() = "enterprise".to_owned();
    cx.update(|window, cx| window.draw(cx).clear());
    let enterprise = cx
        .debug_bounds("radio-group:controlled-radio:item:enterprise")
        .expect("committed Enterprise radio item should remain rendered");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(
        observations.borrow().len(),
        2,
        "activating the live committed selection must not emit a duplicate intent"
    );
}

#[open_gpui::test]
fn radio_group_read_only_semantics_preserve_focus_without_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<RadioSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            RadioGroup::new("read-only-radio")
                .label("Read-only radio")
                .read_only(true)
                .default_selected("personal")
                .item(RadioItem::new("personal", "Read-only personal"))
                .item(RadioItem::new("enterprise", "Read-only enterprise"))
                .on_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(selection);
                })
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("read-only radio group should publish an accessibility tree");
    let (personal_node, personal) = a11y_node_with_label(&update, "Read-only personal");
    let (enterprise_node, enterprise) = a11y_node_with_label(&update, "Read-only enterprise");
    assert!(personal.is_read_only());
    assert!(enterprise.is_read_only());
    assert!(personal.supports_action(accesskit::Action::Focus));
    assert!(!personal.supports_action(accesskit::Action::Click));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: personal_node,
        data: None,
    }));
    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_down.propagated());
    assert!(!space_down.default_prevented());
    assert!(space_up.propagated());
    assert!(!space_up.default_prevented());

    let enterprise_bounds = cx
        .debug_bounds("radio-group:read-only-radio:item:enterprise")
        .expect("read-only Enterprise radio item should be rendered");
    cx.simulate_click(enterprise_bounds.center(), Modifiers::none());
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: enterprise_node,
        data: None,
    }));
    assert!(selections.borrow().is_empty());

    let state = RadioGroup::new("read-only-state")
        .read_only(true)
        .default_selected("personal")
        .item(RadioItem::new("personal", "Personal"))
        .item(RadioItem::new("enterprise", "Enterprise"))
        .state();
    assert!(state.read_only());
    assert!(!state.activation_enabled());
    assert!(state.tab_stop_index().is_some());
    assert!(state.items().iter().all(RadioItemState::read_only));
    assert!(state.items().iter().all(|item| !item.activation_enabled()));
}

#[test]
fn radio_group_state_exposes_selection_required_and_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Vertical,
        Size::Medium,
        false,
        true,
        RadioSelectionAuthority::Uncontrolled(Some("team")),
        None,
        [
            RadioItemDescriptor::new("personal", "Personal"),
            RadioItemDescriptor::new("team", "Team"),
            RadioItemDescriptor::new("enterprise", "Enterprise").disabled(true),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::RadioGroup);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("team"));
    assert_eq!(state.focused_value(), Some("team"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[0].role(), Role::RadioButton);
}

#[test]
fn radio_group_reuses_roving_focus_helpers_and_skips_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        false,
        RadioSelectionAuthority::Uncontrolled(Some("missing")),
        Some("enterprise"),
        [
            RadioItemDescriptor::new("starter", "Starter"),
            RadioItemDescriptor::new("pro", "Pro").disabled(true),
            RadioItemDescriptor::new("enterprise", "Enterprise"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("enterprise"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].disabled());
    assert!(!state.items()[1].focused());
}

#[test]
fn radio_group_builder_state_falls_back_to_first_enabled_item() {
    let state = RadioGroup::new("plan")
        .label("Plan")
        .orientation(Orientation::Horizontal)
        .with_size(Size::Large)
        .required(true)
        .default_selected("enterprise")
        .item(RadioItem::new("starter", "Starter"))
        .item(RadioItem::new("pro", "Pro"))
        .item(RadioItem::new("enterprise", "Enterprise").disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Large);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("starter"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn radio_group_controlled_selection_preserves_exact_owner_projection() {
    for (id, value, expected) in [
        ("controlled-none", None, None),
        ("controlled-removed", Some("removed"), None),
        ("controlled-disabled", Some("team"), Some("team")),
    ] {
        let component_state = RadioGroup::new(id)
            .selected(value.map(str::to_owned))
            .default_selected("enterprise")
            .item(RadioItem::new("personal", "Personal"))
            .item(RadioItem::new("team", "Team").disabled(true))
            .item(RadioItem::new("enterprise", "Enterprise"))
            .state();
        let resolved_state = RadioGroupState::resolve(
            Orientation::Vertical,
            Size::Medium,
            false,
            false,
            RadioSelectionAuthority::Controlled(value),
            None,
            [
                RadioItemDescriptor::new("personal", "Personal"),
                RadioItemDescriptor::new("team", "Team").disabled(true),
                RadioItemDescriptor::new("enterprise", "Enterprise"),
            ],
            ThemeTokens::default(),
        );

        assert_eq!(resolved_state, component_state);
        assert_eq!(component_state.selected_value(), expected);
        assert_eq!(component_state.focused_value(), Some("personal"));
        if value == Some("team") {
            assert!(component_state.items()[1].selected());
            assert!(component_state.items()[1].disabled());
        }
    }
}

#[open_gpui::test]
fn radio_group_programmatic_handles_follow_item_lifecycle_and_gate(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected: Rc<RefCell<Option<String>>>,
        show_removed: Rc<RefCell<bool>>,
        selections: Rc<RefCell<Vec<RadioSelection>>>,
        enterprise_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
        removed_handle: ActivationHandle,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let mut group = RadioGroup::new("programmatic-radio")
                .label("Programmatic radio")
                .selected(self.selected.borrow().clone())
                .item(RadioItem::new("personal", "Programmatic personal"))
                .item(RadioItem::new("enterprise", "Programmatic enterprise"))
                .item(RadioItem::new("disabled", "Programmatic disabled").disabled(true))
                .activation_handle("enterprise", &self.enterprise_handle)
                .activation_handle("disabled", &self.disabled_handle)
                .activation_handle("removed", &self.removed_handle)
                .on_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(selection);
                });

            if *self.show_removed.borrow() {
                group = group.item(RadioItem::new("removed", "Programmatic removed"));
            }

            group
        }
    }

    let selected = Rc::new(RefCell::new(None));
    let show_removed = Rc::new(RefCell::new(true));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let enterprise_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let removed_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selected: selected.clone(),
        show_removed: show_removed.clone(),
        selections: selections.clone(),
        enterprise_handle: enterprise_handle.clone(),
        disabled_handle: disabled_handle.clone(),
        removed_handle: removed_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let enterprise = cx
        .debug_bounds("radio-group:programmatic-radio:item:enterprise")
        .expect("controlled Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(selected.borrow().as_ref(), None);
    assert_eq!(selections.borrow().len(), 1);

    cx.update(|window, cx| {
        assert_eq!(
            enterprise_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(selected.borrow().as_ref(), None);
    assert_eq!(selections.borrow().len(), 2);
    assert!(
        selections
            .borrow()
            .iter()
            .all(|selection| selection.value() == "enterprise")
    );

    *selected.borrow_mut() = Some("removed".to_owned());
    *show_removed.borrow_mut() = false;
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        assert_eq!(
            removed_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
    let enterprise = cx
        .debug_bounds("radio-group:programmatic-radio:item:enterprise")
        .expect("Enterprise radio item should survive removal of its sibling");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(selected.borrow().as_deref(), Some("removed"));
    assert_eq!(selections.borrow().len(), 3);

    *selected.borrow_mut() = Some("disabled".to_owned());
    cx.update(|window, cx| window.draw(cx).clear());
    let enterprise = cx
        .debug_bounds("radio-group:programmatic-radio:item:enterprise")
        .expect("Enterprise radio item should remain enabled");
    cx.simulate_click(enterprise.center(), Modifiers::none());
    assert_eq!(selected.borrow().as_deref(), Some("disabled"));
    assert_eq!(selections.borrow().len(), 4);
}
