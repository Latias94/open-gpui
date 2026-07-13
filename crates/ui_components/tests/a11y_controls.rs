use open_gpui::{Context, IntoElement, ParentElement, Render, Window, accesskit, div};
use open_gpui_ui_components::{
    Checkbox, IconButton, NumberInput, NumberInputChange, NumberInputStepAction, Progress, Slider,
    SliderChange, Switch, Toggle,
};
use open_gpui_ui_core::Toggled;
use std::{cell::RefCell, rc::Rc};

fn a11y_node_with_label<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing accessibility node labelled {label:?}"))
}

fn action_request(
    action: accesskit::Action,
    target_node: accesskit::NodeId,
    data: Option<accesskit::ActionData>,
) -> accesskit::ActionRequest {
    accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data,
    }
}

#[open_gpui::test]
fn action_controls_project_exact_semantics_and_dispatch(cx: &mut open_gpui::TestAppContext) {
    struct ActionControlsProbe {
        icon_disabled: bool,
        switch_checked: bool,
        toggle_pressed: bool,
        icon_activations: Rc<RefCell<usize>>,
        switch_changes: Rc<RefCell<Vec<bool>>>,
        toggle_changes: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for ActionControlsProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let icon_activations = self.icon_activations.clone();
            let switch_changes = self.switch_changes.clone();
            let toggle_changes = self.toggle_changes.clone();

            div()
                .child(
                    IconButton::new("semantic-icon-button", "?", "Search")
                        .selected(true)
                        .disabled(self.icon_disabled)
                        .accessibility_description("Search documents")
                        .on_click(move |_, _, _| *icon_activations.borrow_mut() += 1),
                )
                .child(IconButton::new(
                    "semantic-passive-icon-button",
                    "i",
                    "Information",
                ))
                .child(
                    Switch::new("semantic-switch")
                        .label("Auto save")
                        .checked(self.switch_checked)
                        .on_change(move |checked, _, _, _| {
                            switch_changes.borrow_mut().push(checked);
                        }),
                )
                .child(
                    Toggle::new("semantic-toggle", "Bold")
                        .pressed(self.toggle_pressed)
                        .on_change(move |pressed, _, _, _| {
                            toggle_changes.borrow_mut().push(pressed);
                        }),
                )
        }
    }

    let icon_activations = Rc::new(RefCell::new(0));
    let switch_changes = Rc::new(RefCell::new(Vec::new()));
    let toggle_changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| ActionControlsProbe {
        icon_disabled: false,
        switch_checked: false,
        toggle_pressed: true,
        icon_activations: icon_activations.clone(),
        switch_changes: switch_changes.clone(),
        toggle_changes: toggle_changes.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("action controls should publish their final accessibility tree");
    let (icon_id, icon) = a11y_node_with_label(&initial, "Search");
    assert_eq!(icon.role(), accesskit::Role::Button);
    assert_eq!(icon.description(), Some("Search documents"));
    assert_eq!(icon.is_selected(), Some(true));
    assert!(!icon.is_disabled());
    assert!(icon.supports_action(accesskit::Action::Click));
    assert!(icon.supports_action(accesskit::Action::Focus));

    let (passive_icon_id, passive_icon) = a11y_node_with_label(&initial, "Information");
    assert!(!passive_icon.supports_action(accesskit::Action::Click));
    assert!(passive_icon.supports_action(accesskit::Action::Focus));

    let (switch_id, switch) = a11y_node_with_label(&initial, "Auto save");
    assert_eq!(switch.role(), accesskit::Role::Switch);
    assert_eq!(switch.toggled(), Some(accesskit::Toggled::False));
    assert!(switch.supports_action(accesskit::Action::Click));
    assert!(switch.supports_action(accesskit::Action::Focus));

    let (toggle_id, toggle) = a11y_node_with_label(&initial, "Bold");
    assert_eq!(toggle.role(), accesskit::Role::Button);
    assert_eq!(toggle.toggled(), Some(accesskit::Toggled::True));
    assert!(toggle.supports_action(accesskit::Action::Click));
    assert!(toggle.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        icon_id,
        None,
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        switch_id,
        None,
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        toggle_id,
        None,
    )));
    assert_eq!(*icon_activations.borrow(), 1);
    assert_eq!(switch_changes.borrow().as_slice(), &[true]);
    assert_eq!(toggle_changes.borrow().as_slice(), &[false]);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        passive_icon_id,
        None,
    )));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("focus action should publish")
            .focus,
        passive_icon_id
    );

    view.update(cx, |probe, cx| {
        probe.icon_disabled = true;
        probe.switch_checked = true;
        probe.toggle_pressed = false;
        cx.notify();
    });
    cx.run_until_parked();

    let updated = cx
        .latest_accessibility_tree_update()
        .expect("updated action controls should publish");
    let (updated_icon_id, updated_icon) = a11y_node_with_label(&updated, "Search");
    assert_eq!(updated_icon_id, icon_id);
    assert!(updated_icon.is_disabled());
    assert!(!updated_icon.supports_action(accesskit::Action::Click));
    assert!(!updated_icon.supports_action(accesskit::Action::Focus));
    let (updated_switch_id, updated_switch) = a11y_node_with_label(&updated, "Auto save");
    assert_eq!(updated_switch_id, switch_id);
    assert_eq!(updated_switch.toggled(), Some(accesskit::Toggled::True));
    let (updated_toggle_id, updated_toggle) = a11y_node_with_label(&updated, "Bold");
    assert_eq!(updated_toggle_id, toggle_id);
    assert_eq!(updated_toggle.toggled(), Some(accesskit::Toggled::False));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        icon_id,
        None,
    )));
    assert_eq!(*icon_activations.borrow(), 1);
}

#[open_gpui::test]
fn checkbox_final_tree_tracks_form_state_actions_and_stable_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    struct CheckboxProbe {
        toggled: Toggled,
        disabled: bool,
        required: bool,
        invalid: bool,
        busy: bool,
        show: bool,
        changes: Rc<RefCell<Vec<Toggled>>>,
    }

    impl Render for CheckboxProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let checkbox = Checkbox::new("semantic-checkbox")
                .label("Accept terms")
                .checked_state(self.toggled)
                .disabled(self.disabled)
                .required(self.required)
                .invalid(self.invalid)
                .busy(self.busy)
                .on_toggle(move |toggled, _, _, _| changes.borrow_mut().push(toggled));

            if self.show {
                div().child(checkbox)
            } else {
                div()
            }
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| CheckboxProbe {
        toggled: Toggled::Mixed,
        disabled: false,
        required: true,
        invalid: true,
        busy: true,
        show: true,
        changes: changes.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("checkbox should publish its final accessibility tree");
    let (checkbox_id, checkbox) = a11y_node_with_label(&initial, "Accept terms");
    assert_eq!(checkbox.role(), accesskit::Role::CheckBox);
    assert_eq!(checkbox.toggled(), Some(accesskit::Toggled::Mixed));
    assert_eq!(checkbox.invalid(), Some(accesskit::Invalid::True));
    assert!(checkbox.is_required());
    assert!(checkbox.is_busy());
    assert!(!checkbox.is_disabled());
    assert!(checkbox.supports_action(accesskit::Action::Click));
    assert!(checkbox.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        checkbox_id,
        None,
    )));
    assert_eq!(changes.borrow().as_slice(), &[Toggled::True]);

    view.update(cx, |probe, cx| {
        probe.toggled = Toggled::True;
        probe.disabled = true;
        probe.required = false;
        probe.invalid = false;
        probe.busy = false;
        cx.notify();
    });
    cx.run_until_parked();

    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled checkbox should publish");
    let (disabled_id, disabled_checkbox) = a11y_node_with_label(&disabled, "Accept terms");
    assert_eq!(disabled_id, checkbox_id);
    assert_eq!(disabled_checkbox.toggled(), Some(accesskit::Toggled::True));
    assert_eq!(disabled_checkbox.invalid(), None);
    assert!(!disabled_checkbox.is_required());
    assert!(!disabled_checkbox.is_busy());
    assert!(disabled_checkbox.is_disabled());
    assert!(!disabled_checkbox.supports_action(accesskit::Action::Click));
    assert!(!disabled_checkbox.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Click,
        checkbox_id,
        None,
    )));
    assert_eq!(changes.borrow().as_slice(), &[Toggled::True]);

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("checkbox unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == checkbox_id));
}

#[open_gpui::test]
fn numeric_controls_project_real_set_value_and_same_node_updates(
    cx: &mut open_gpui::TestAppContext,
) {
    struct NumericControlsProbe {
        slider_value: f32,
        slider_disabled: bool,
        number_value: f32,
        number_read_only: bool,
        number_invalid: bool,
        number_required: bool,
        number_busy: bool,
        progress_value: Option<f32>,
        slider_changes: Rc<RefCell<Vec<SliderChange>>>,
        number_changes: Rc<RefCell<Vec<NumberInputChange>>>,
    }

    impl Render for NumericControlsProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let slider_changes = self.slider_changes.clone();
            let number_changes = self.number_changes.clone();
            let progress = match self.progress_value {
                Some(value) => Progress::new("semantic-progress", "Build progress").value(value),
                None => Progress::new("semantic-progress", "Build progress").indeterminate(),
            };

            div()
                .child(
                    Slider::new("semantic-slider", "Volume")
                        .min(0.0)
                        .max(100.0)
                        .step(5.0)
                        .value(self.slider_value)
                        .disabled(self.slider_disabled)
                        .on_change(move |change, _, _| {
                            slider_changes.borrow_mut().push(change);
                        }),
                )
                .child(
                    NumberInput::new("semantic-number-input", "Quantity")
                        .min(0.0)
                        .max(10.0)
                        .step(1.0)
                        .value(self.number_value)
                        .read_only(self.number_read_only)
                        .invalid(self.number_invalid)
                        .required(self.number_required)
                        .busy(self.number_busy)
                        .on_change(move |change, _, _| {
                            number_changes.borrow_mut().push(change);
                        }),
                )
                .child(progress)
        }
    }

    let slider_changes = Rc::new(RefCell::new(Vec::new()));
    let number_changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| NumericControlsProbe {
        slider_value: 40.0,
        slider_disabled: false,
        number_value: 3.0,
        number_read_only: false,
        number_invalid: true,
        number_required: true,
        number_busy: true,
        progress_value: Some(70.0),
        slider_changes: slider_changes.clone(),
        number_changes: number_changes.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("numeric controls should publish their final accessibility tree");
    let (slider_id, slider) = a11y_node_with_label(&initial, "Volume");
    assert_eq!(slider.role(), accesskit::Role::Slider);
    assert_eq!(slider.numeric_value(), Some(40.0));
    assert_eq!(slider.min_numeric_value(), Some(0.0));
    assert_eq!(slider.max_numeric_value(), Some(100.0));
    assert_eq!(
        slider.orientation(),
        Some(accesskit::Orientation::Horizontal)
    );
    for action in [
        accesskit::Action::Focus,
        accesskit::Action::Increment,
        accesskit::Action::Decrement,
        accesskit::Action::SetValue,
    ] {
        assert!(slider.supports_action(action));
    }

    let (number_id, number) = a11y_node_with_label(&initial, "Quantity");
    assert_eq!(number.role(), accesskit::Role::SpinButton);
    assert_eq!(number.numeric_value(), Some(3.0));
    assert_eq!(number.min_numeric_value(), Some(0.0));
    assert_eq!(number.max_numeric_value(), Some(10.0));
    assert_eq!(number.invalid(), Some(accesskit::Invalid::True));
    assert!(number.is_required());
    assert!(number.is_busy());
    assert!(!number.is_read_only());
    for action in [
        accesskit::Action::Focus,
        accesskit::Action::Increment,
        accesskit::Action::Decrement,
        accesskit::Action::SetValue,
    ] {
        assert!(number.supports_action(action));
    }

    let (progress_id, progress) = a11y_node_with_label(&initial, "Build progress");
    assert_eq!(progress.role(), accesskit::Role::ProgressIndicator);
    assert_eq!(progress.numeric_value(), Some(70.0));
    assert_eq!(progress.min_numeric_value(), Some(0.0));
    assert_eq!(progress.max_numeric_value(), Some(100.0));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        slider_id,
        Some(accesskit::ActionData::NumericValue(73.0)),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        number_id,
        Some(accesskit::ActionData::NumericValue(8.4)),
    )));
    assert_eq!(slider_changes.borrow().len(), 1);
    assert_eq!(slider_changes.borrow()[0].previous_value(), 40.0);
    assert_eq!(slider_changes.borrow()[0].value(), 75.0);
    assert_eq!(number_changes.borrow().len(), 1);
    assert_eq!(
        number_changes.borrow()[0].action(),
        NumberInputStepAction::SetValue
    );
    assert_eq!(number_changes.borrow()[0].previous_value(), 3.0);
    assert_eq!(number_changes.borrow()[0].value(), 8.0);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        number_id,
        Some(accesskit::ActionData::Value("9".into())),
    )));
    assert_eq!(number_changes.borrow().len(), 1);

    view.update(cx, |probe, cx| {
        probe.slider_value = 75.0;
        probe.slider_disabled = true;
        probe.number_value = 8.0;
        probe.number_read_only = true;
        probe.number_invalid = false;
        probe.number_required = false;
        probe.number_busy = false;
        probe.progress_value = None;
        cx.notify();
    });
    cx.run_until_parked();

    let updated = cx
        .latest_accessibility_tree_update()
        .expect("updated numeric controls should publish");
    let (updated_slider_id, updated_slider) = a11y_node_with_label(&updated, "Volume");
    assert_eq!(updated_slider_id, slider_id);
    assert_eq!(updated_slider.numeric_value(), Some(75.0));
    assert!(updated_slider.is_disabled());
    for action in [
        accesskit::Action::Focus,
        accesskit::Action::Increment,
        accesskit::Action::Decrement,
        accesskit::Action::SetValue,
    ] {
        assert!(!updated_slider.supports_action(action));
    }

    let (updated_number_id, updated_number) = a11y_node_with_label(&updated, "Quantity");
    assert_eq!(updated_number_id, number_id);
    assert_eq!(updated_number.numeric_value(), Some(8.0));
    assert!(updated_number.is_read_only());
    assert_eq!(updated_number.invalid(), None);
    assert!(!updated_number.is_required());
    assert!(!updated_number.is_busy());
    assert!(updated_number.supports_action(accesskit::Action::Focus));
    assert!(!updated_number.supports_action(accesskit::Action::Increment));
    assert!(!updated_number.supports_action(accesskit::Action::Decrement));
    assert!(!updated_number.supports_action(accesskit::Action::SetValue));

    let (updated_progress_id, updated_progress) = a11y_node_with_label(&updated, "Build progress");
    assert_eq!(updated_progress_id, progress_id);
    assert_eq!(updated_progress.numeric_value(), None);
    assert_eq!(updated_progress.min_numeric_value(), Some(0.0));
    assert_eq!(updated_progress.max_numeric_value(), Some(100.0));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        slider_id,
        Some(accesskit::ActionData::NumericValue(10.0)),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        number_id,
        Some(accesskit::ActionData::NumericValue(2.0)),
    )));
    assert_eq!(slider_changes.borrow().len(), 1);
    assert_eq!(number_changes.borrow().len(), 1);
}
