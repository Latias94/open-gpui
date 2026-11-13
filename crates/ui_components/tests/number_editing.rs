use open_gpui::{
    AppContext, Context, Entity, EntityInputHandler, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, ParentElement, PointerCancelReason, Render,
    Styled, TestAppContext, VisualTestContext, Window, accesskit, div, point, px,
};
use open_gpui_ui_components::{
    TextInput,
    gpui_adapter::{TextInputController, init_text_input},
    number_input::{
        NumberInput, NumberInputCancel, NumberInputCancelReason, NumberInputChange,
        NumberInputCommit, NumberInputController, NumberInputDraftValue, NumberInputEditSource,
        NumberInputPreview,
    },
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct Events {
    previews: Vec<NumberInputPreview>,
    changes: Vec<NumberInputChange>,
    commits: Vec<NumberInputCommit>,
    cancels: Vec<NumberInputCancel>,
    parent_presses: usize,
    form_submits: usize,
}

struct Probe {
    controller: Entity<NumberInputController>,
    sibling: Entity<TextInputController>,
    value: Option<f32>,
    accept: bool,
    disabled: bool,
    read_only: bool,
    visible: bool,
    scrub_visible: bool,
    events: Rc<RefCell<Events>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let previews = self.events.clone();
        let commits = self.events.clone();
        let cancels = self.events.clone();
        let parent_presses = self.events.clone();
        let form_submits = self.events.clone();
        let mut number = NumberInput::new("scalar", "Quantity")
            .controller(self.controller.clone())
            .min(-10.0)
            .max(10.0)
            .step(0.5)
            .disabled(self.disabled)
            .read_only(self.read_only)
            .scrub(self.scrub_visible)
            .scrub_pixels_per_step(8.0)
            .on_preview(move |preview, _, _| previews.borrow_mut().previews.push(preview))
            .on_change(move |change, _, cx| {
                let _ = owner.update(cx, |probe, cx| {
                    probe.events.borrow_mut().changes.push(change);
                    if probe.accept && probe.value.is_some() {
                        probe.value = Some(change.value());
                    }
                    cx.notify();
                });
            })
            .on_commit(move |result, _, _| commits.borrow_mut().commits.push(result))
            .on_cancel(move |cancel, _, _| cancels.borrow_mut().cancels.push(cancel));
        if let Some(value) = self.value {
            number = number.value(value);
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                parent_presses.borrow_mut().parent_presses += 1;
            })
            .on_key_down(move |event, window, _| {
                if event.keystroke.key == "enter"
                    && !event.prefer_character_input
                    && !window.default_prevented()
                {
                    form_submits.borrow_mut().form_submits += 1;
                }
            })
            .children(self.visible.then_some(number))
            .child(TextInput::new("sibling", "Sibling").controller(self.sibling.clone()))
    }
}

fn probe(
    cx: &mut TestAppContext,
    controlled: bool,
) -> (
    Entity<Probe>,
    Entity<NumberInputController>,
    Rc<RefCell<Events>>,
    &mut VisualTestContext,
) {
    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Events::default()));
    let (view, cx) = cx.add_window_view(|window, cx| Probe {
        controller: cx.new(|cx| NumberInputController::new(4.0, window, cx)),
        sibling: cx.new(|cx| TextInputController::new(cx)),
        value: controlled.then_some(4.0),
        accept: true,
        disabled: false,
        read_only: false,
        visible: true,
        scrub_visible: true,
        events: events.clone(),
    });
    let _ = cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let controller = cx.update(|window, cx| {
        assert!(window.is_window_active());
        window.draw(cx).clear(cx);
        let controller = view.read(cx).controller.clone();
        window.focus(&controller.focus_handle(cx), cx);
        window.draw(cx).clear(cx);
        controller
    });
    (view, controller, events, cx)
}

fn replace_draft(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input(text);
}

fn key_down(key: &str, prefer_character_input: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input,
    }
}

#[test]
fn decimal_drafts_distinguish_incomplete_invalid_and_finite_input() {
    for text in ["", " "] {
        assert_eq!(
            NumberInputDraftValue::parse(text),
            NumberInputDraftValue::Empty
        );
    }
    for text in ["-", "+", ".", "-.", "1.", "-1e", "1e-", "1E+"] {
        assert_eq!(
            NumberInputDraftValue::parse(text),
            NumberInputDraftValue::Incomplete,
            "{text}"
        );
    }
    for text in ["NaN", "inf", "1e100", "--1", "1.2.3", "3px"] {
        assert_eq!(
            NumberInputDraftValue::parse(text),
            NumberInputDraftValue::Invalid,
            "{text}"
        );
    }
    assert_eq!(
        NumberInputDraftValue::parse(" -2.5e1 "),
        NumberInputDraftValue::Value(-25.0)
    );
    let state = NumberInput::new("extremes", "Extremes")
        .min(-f32::MAX)
        .max(f32::MAX)
        .step(1.0)
        .value(f32::MAX)
        .state();
    assert!(state.value().is_finite());
    let small = NumberInput::new("small", "Small")
        .step(1e-9)
        .value(1e-9)
        .state();
    assert_eq!(small.value(), 1e-9);
    assert_ne!(small.display_value(), "0");
}

#[open_gpui::test]
fn text_drafts_survive_render_and_only_enter_submits(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "-");
    cx.update(|_, cx| {
        assert_eq!(
            controller.read(cx).draft().parsed(),
            NumberInputDraftValue::Incomplete
        );
        assert_eq!(controller.read(cx).value(), 4.0);
    });
    replace_draft(cx, "1.");
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let controller = controller.read(cx);
        assert_eq!(controller.draft().text(), "1.");
        assert_eq!(controller.text_input().read(cx).value(), "1.");
        assert_eq!(controller.text_input().read(cx).selected_range(), 2..2);
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    replace_draft(cx, "7.2");
    assert!(events.borrow().changes.is_empty());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().changes[0].value(), 7.0);
    assert_eq!(events.borrow().commits.len(), 1);
    assert!(events.borrow().commits[0].accepted());
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Text
    );
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).draft().text(), "7");
        assert_eq!(controller.read(cx).value(), 7.0);
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(events.borrow().commits.len(), 1);
}

#[open_gpui::test]
fn escape_and_invalid_submission_restore_the_accepted_value(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "5.5");
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).draft().text(), "4");
        assert!(!controller.read(cx).draft().is_dirty());
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    assert_eq!(events.borrow().cancels.len(), 1);
    assert_eq!(
        events.borrow().cancels[0].reason(),
        NumberInputCancelReason::Escape
    );
    replace_draft(cx, "-");
    cx.simulate_keystrokes("enter");
    assert_eq!(events.borrow().cancels.len(), 2);
    assert_eq!(
        events.borrow().cancels[1].reason(),
        NumberInputCancelReason::InvalidDraft
    );
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    assert_eq!(events.borrow().form_submits, 0);
}

#[open_gpui::test]
fn a_trailing_decimal_point_is_only_finalized_when_submitted(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "1.");
    assert!(events.borrow().changes.is_empty());
    cx.update(|_, cx| assert_eq!(controller.read(cx).draft().text(), "1."));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
    assert!(events.borrow().commits[0].accepted());
    cx.update(|_, cx| {
        assert_eq!(controller.read(cx).draft().text(), "1");
        assert_eq!(controller.read(cx).value(), 1.0);
    });
}

#[open_gpui::test]
fn rejected_commit_reconciles_once_and_retains_selection_and_focus(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    view.update(cx, |probe, cx| {
        probe.accept = false;
        cx.notify();
    });
    replace_draft(cx, "9.5");
    cx.simulate_keystrokes("home shift-right enter");
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
    let result = events.borrow().commits[0];
    assert!(!result.accepted());
    assert_eq!(result.proposed_value(), 9.5);
    assert_eq!(result.value(), 4.0);
    cx.update(|window, cx| {
        let controller = controller.read(cx);
        assert_eq!(controller.draft().text(), "4");
        assert_eq!(controller.text_input().read(cx).selected_range(), 0..1);
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(events.borrow().commits.len(), 1);
}

#[open_gpui::test]
fn external_value_replaces_dirty_text_but_unchanged_values_preserve_it(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "1e-");
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(controller.read(cx).draft().text(), "1e-"));
    assert!(events.borrow().cancels.is_empty());
    view.update(cx, |probe, cx| {
        probe.value = Some(-2.5);
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), -2.5);
        assert_eq!(controller.read(cx).draft().text(), "-2.5");
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    assert_eq!(events.borrow().cancels.len(), 1);
    assert_eq!(
        events.borrow().cancels[0].reason(),
        NumberInputCancelReason::ExternalValue
    );
    assert!(events.borrow().changes.is_empty());
}

#[open_gpui::test]
fn composition_enter_and_arrows_stay_with_the_native_text_owner(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    cx.simulate_marked_text(Some(0..1), "6", Some(0..1));
    for key in ["enter", "up", "down"] {
        cx.simulate_event(key_down(key, false));
    }
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    cx.update(|window, cx| {
        let controller = controller.read(cx);
        assert_eq!(
            controller.text_input().read(cx).marked_range_utf16(),
            Some(0..1)
        );
        assert_eq!(
            controller.text_input().read(cx).selected_range_utf16(),
            0..1
        );
        assert!(controller.focus_handle(cx).is_focused(window));
    });
    replace_draft(cx, "6.5");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(events.borrow().commits.len(), 1);
    assert!(events.borrow().commits[0].accepted());
}

#[open_gpui::test]
fn ime_confirmation_and_prevented_enter_do_not_submit_an_unmarked_draft(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "6.5");
    cx.simulate_event(key_down("enter", true));
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    let interceptor = cx.update(|window, _| {
        window.intercept_window_key_down(|_, window, _| window.prevent_default())
    });
    cx.simulate_event(key_down("enter", false));
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    assert_eq!(events.borrow().form_submits, 0);
    cx.update(|_, cx| assert_eq!(controller.read(cx).draft().text(), "6.5"));
    drop(interceptor);
    cx.simulate_event(key_down("enter", false));
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
}

#[open_gpui::test]
fn canceling_a_submission_before_delivery_prevents_its_change_callback(cx: &mut TestAppContext) {
    for controlled in [true, false] {
        let (_, controller, events, cx) = probe(cx, controlled);
        replace_draft(cx, "7");
        cx.update(|window, cx| {
            controller.update(cx, |controller, cx| {
                assert!(controller.commit(window, cx));
                assert!(controller.cancel(NumberInputCancelReason::Escape, window, cx));
            });
        });
        cx.run_until_parked();
        assert!(events.borrow().changes.is_empty());
        assert!(events.borrow().commits.is_empty());
        assert_eq!(events.borrow().cancels.len(), 1);
        cx.update(|_, cx| {
            assert_eq!(controller.read(cx).value(), 4.0);
            assert_eq!(controller.read(cx).draft().text(), "4");
        });
    }
}

#[open_gpui::test]
fn rejected_submission_and_canceled_scrub_preserve_reversed_selection(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    view.update(cx, |probe, cx| {
        probe.accept = false;
        cx.notify();
    });
    replace_draft(cx, "9.5");
    cx.simulate_keystrokes("end shift-left shift-left shift-left enter");
    cx.run_until_parked();
    assert_eq!(events.borrow().commits.len(), 1);
    assert!(!events.borrow().commits[0].accepted());
    cx.update(|window, cx| {
        let input = controller.read(cx).text_input().clone();
        let selection = input.update(cx, |input, cx| {
            input
                .selected_text_range(true, window, cx)
                .expect("selection")
        });
        assert_eq!(selection.range, 0..1);
        assert!(selection.reversed);
    });
    replace_draft(cx, "1.");
    cx.simulate_keystrokes("end shift-left shift-left");
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("scrub handle")
        .center();
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).draft().text(), "1.");
        let input = controller.read(cx).text_input().clone();
        let selection = input.update(cx, |input, cx| {
            input
                .selected_text_range(true, window, cx)
                .expect("selection")
        });
        assert_eq!(selection.range, 0..2);
        assert!(selection.reversed);
    });
    assert_eq!(events.borrow().changes.len(), 1);
}

#[open_gpui::test]
fn blocked_stepper_and_scrub_presses_reach_the_parent_row(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    for disabled in [false, true] {
        view.update(cx, |probe, cx| {
            probe.read_only = !disabled;
            probe.disabled = disabled;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in [
            "number-input:scalar:number-step-up",
            "number-input:scalar:scrub",
        ] {
            let position = cx.debug_bounds(selector).expect("affordance").center();
            let previous = events.borrow().parent_presses;
            cx.simulate_mouse_down(position, MouseButton::Left, Default::default());
            assert_eq!(events.borrow().parent_presses, previous + 1);
            cx.simulate_mouse_up(position, MouseButton::Left, Default::default());
        }
    }
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 4.0);
        assert!(!controller.read(cx).is_scrubbing());
        assert!(window.captured_pointer().is_none());
    });
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
}

#[open_gpui::test]
fn stepper_accessibility_click_uses_the_owned_button_transaction(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    assert!(cx.activate_accessibility());
    let tree = cx.latest_accessibility_tree_update().expect("number tree");
    let (button_id, button) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Increment Quantity"))
        .expect("increment button");
    assert_eq!(button.role(), accesskit::Role::Button);
    assert!(button.supports_action(accesskit::Action::Click));
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *button_id,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Accessibility
    );
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 4.5);
        assert!(controller.focus_handle(cx).is_focused(window));
    });
}

#[open_gpui::test]
fn scrub_release_uses_its_final_position_without_a_move_event(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("scrub handle")
        .center();
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        assert!(controller.read(cx).is_scrubbing());
        assert!(window.captured_pointer().is_some());
    });
    cx.simulate_mouse_up(
        point(start.x + px(16.0), start.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().changes[0].value(), 5.0);
    assert_eq!(events.borrow().commits.len(), 1);
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 5.0);
        assert!(!controller.read(cx).is_scrubbing());
        assert!(window.captured_pointer().is_none());
    });
    replace_draft(cx, "1.");
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(start, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(controller.read(cx).draft().text(), "1."));
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
}

#[open_gpui::test]
fn focus_transfer_commits_once_and_does_not_refocus_the_number(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "5.5");
    cx.update(|window, cx| {
        let sibling = view.read(cx).sibling.clone();
        window.focus(&sibling.focus_handle(cx), cx);
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 5.5);
        assert!(!controller.focus_handle(cx).is_focused(window));
        assert!(view.read(cx).sibling.focus_handle(cx).is_focused(window));
    });
}

#[open_gpui::test]
fn local_value_works_without_a_controlled_echo_and_steps_from_the_draft(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, false);
    replace_draft(cx, "2.5");
    cx.simulate_keystrokes("up");
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().changes[0].value(), 3.0);
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Keyboard
    );
    cx.update(|_, cx| assert_eq!(controller.read(cx).value(), 3.0));
    cx.simulate_keystrokes("down");
    cx.update(|_, cx| assert_eq!(controller.read(cx).value(), 2.5));
    assert_eq!(events.borrow().commits.len(), 2);
}

#[open_gpui::test]
fn pointer_stepper_submits_once_without_disturbing_text_focus(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    let stepper = cx
        .debug_bounds("number-input:scalar:number-step-up")
        .expect("increment button")
        .center();
    cx.simulate_mouse_down(stepper, MouseButton::Left, Default::default());
    assert!(events.borrow().changes.is_empty());
    cx.simulate_mouse_up(stepper, MouseButton::Left, Default::default());
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Stepper
    );
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 4.5);
        assert!(controller.focus_handle(cx).is_focused(window));
    });
}

#[open_gpui::test]
fn disabled_and_read_only_gate_native_text_keyboard_and_accessibility(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "7");
    view.update(cx, |probe, cx| {
        probe.read_only = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_input("9");
    cx.simulate_keystrokes("up enter");
    cx.update(|_, cx| {
        let controller = controller.read(cx);
        assert_eq!(controller.draft().text(), "4");
        assert!(controller.text_input().read(cx).read_only());
    });
    assert_eq!(
        events.borrow().cancels[0].reason(),
        NumberInputCancelReason::EditingDisabled
    );
    assert!(cx.activate_accessibility());
    let tree = cx.latest_accessibility_tree_update().expect("number tree");
    let (number_id, number) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Quantity"))
        .expect("number node");
    assert!(!number.supports_action(accesskit::Action::Increment));
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Increment,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *number_id,
        data: None,
    }));
    view.update(cx, |probe, cx| {
        probe.read_only = false;
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_input("8");
    cx.simulate_keystrokes("down enter");
    cx.update(|_, cx| assert_eq!(controller.read(cx).value(), 4.0));
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
}

#[open_gpui::test]
fn numeric_accessibility_actions_use_the_same_bounded_proposal_path(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    assert!(cx.activate_accessibility());
    let tree = cx.latest_accessibility_tree_update().expect("number tree");
    let (number_id, number) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Quantity"))
        .expect("number node");
    assert_eq!(number.role(), accesskit::Role::SpinButton);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::SetValue,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *number_id,
        data: Some(accesskit::ActionData::NumericValue(30.0)),
    }));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(controller.read(cx).value(), 10.0));
    assert_eq!(events.borrow().commits.len(), 1);
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Accessibility
    );
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Increment,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *number_id,
        data: None,
    }));
    assert_eq!(events.borrow().commits.len(), 1);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::SetValue,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *number_id,
        data: Some(accesskit::ActionData::NumericValue(f64::INFINITY)),
    }));
    assert_eq!(events.borrow().changes.len(), 1);
}

#[open_gpui::test]
fn captured_scrub_previews_then_commits_once_on_release_outside_the_handle(
    cx: &mut TestAppContext,
) {
    let (_, controller, events, cx) = probe(cx, true);
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("scrub handle")
        .center();
    let end = point(start.x + px(200.0), start.y + px(50.0));
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        assert!(controller.read(cx).is_scrubbing());
        assert!(window.captured_pointer().is_some());
        assert_eq!(controller.read(cx).value(), 4.0);
        assert_eq!(
            controller.read(cx).draft().parsed(),
            NumberInputDraftValue::Value(10.0)
        );
    });
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    assert_eq!(
        events
            .borrow()
            .previews
            .last()
            .expect("scrub preview")
            .source(),
        NumberInputEditSource::Scrub
    );
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.run_until_parked();
    assert_eq!(events.borrow().changes.len(), 1);
    assert_eq!(events.borrow().commits.len(), 1);
    assert_eq!(
        events.borrow().commits[0].source(),
        NumberInputEditSource::Scrub
    );
    cx.update(|window, cx| {
        assert_eq!(controller.read(cx).value(), 10.0);
        assert!(!controller.read(cx).is_scrubbing());
        assert!(window.captured_pointer().is_none());
    });
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    assert_eq!(events.borrow().commits.len(), 1);
}

#[open_gpui::test]
fn escape_and_capture_loss_restore_the_starting_draft_and_selection(cx: &mut TestAppContext) {
    let (_, controller, events, cx) = probe(cx, true);
    replace_draft(cx, "1.");
    cx.simulate_keystrokes("home shift-right");
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("scrub handle")
        .center();
    let end = point(start.x + px(24.0), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        let controller = controller.read(cx);
        assert_eq!(controller.draft().text(), "1.");
        assert_eq!(controller.text_input().read(cx).selected_range(), 0..1);
        assert!(controller.focus_handle(cx).is_focused(window));
        assert!(!controller.is_scrubbing());
        assert!(window.captured_pointer().is_none());
    });
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        window.cancel_pointer_session(PointerCancelReason::PlatformCaptureLost, cx)
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(controller.read(cx).draft().text(), "1."));
    assert_eq!(events.borrow().cancels.len(), 2);
    assert_eq!(
        events.borrow().cancels[1].reason(),
        NumberInputCancelReason::PointerCancelled
    );
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
}

#[open_gpui::test]
fn removing_a_scrub_owner_cancels_capture_without_submitting(cx: &mut TestAppContext) {
    let (view, controller, events, cx) = probe(cx, true);
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("scrub handle")
        .center();
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(start.x + px(16.0), start.y),
        MouseButton::Left,
        Default::default(),
    );
    view.update(cx, |probe, cx| {
        probe.scrub_visible = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(!controller.read(cx).is_scrubbing());
        assert_eq!(controller.read(cx).value(), 4.0);
        assert!(window.captured_pointer().is_none());
    });
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
    assert_eq!(events.borrow().cancels.len(), 1);

    view.update(cx, |probe, cx| {
        probe.scrub_visible = true;
        cx.notify();
    });
    cx.run_until_parked();
    let start = cx
        .debug_bounds("number-input:scalar:scrub")
        .expect("remounted scrub handle")
        .center();
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(start.x + px(16.0), start.y),
        MouseButton::Left,
        Default::default(),
    );
    view.update(cx, |probe, cx| {
        probe.visible = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(!controller.read(cx).is_scrubbing());
        assert_eq!(controller.read(cx).value(), 4.0);
        assert!(window.captured_pointer().is_none());
    });
    assert_eq!(events.borrow().cancels.len(), 2);
    assert!(events.borrow().changes.is_empty());
    assert!(events.borrow().commits.is_empty());
}
