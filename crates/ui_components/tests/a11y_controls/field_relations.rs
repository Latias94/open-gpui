use super::common::*;

#[open_gpui::test]
fn label_projects_descriptor_state_and_stable_lifecycle(cx: &mut open_gpui::TestAppContext) {
    struct LabelProbe {
        disabled: bool,
        show: bool,
    }

    impl Render for LabelProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().when(self.show, |this| {
                this.child(
                    Label::new("semantic-label", "Account label")
                        .required(true)
                        .disabled(self.disabled),
                )
            })
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| LabelProbe {
        disabled: true,
        show: true,
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("label should publish its final accessibility tree");
    let (label_id, label) = a11y_node_with_label(&initial, "Account label");
    assert_eq!(label.role(), accesskit::Role::Label);
    assert!(label.is_disabled());
    assert_exact_actions(label, &[]);

    view.update(cx, |probe, cx| {
        probe.disabled = false;
        cx.notify();
    });
    cx.run_until_parked();

    let enabled = cx
        .latest_accessibility_tree_update()
        .expect("enabled label should publish");
    let (enabled_label_id, enabled_label) = a11y_node_with_label(&enabled, "Account label");
    assert_eq!(enabled_label_id, label_id);
    assert!(!enabled_label.is_disabled());
    assert_exact_actions(enabled_label, &[]);

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();

    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("label unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == label_id));
}

#[open_gpui::test]
fn field_relations_follow_help_error_transitions_and_unmount(cx: &mut open_gpui::TestAppContext) {
    struct FieldProbe {
        invalid: bool,
        show: bool,
    }

    impl Render for FieldProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().when(self.show, |this| {
                this.child(
                    Field::new("semantic-field", "Email")
                        .help("Use a work address")
                        .error("Enter a valid email")
                        .required(true)
                        .invalid(self.invalid)
                        .control(
                            TextInput::new("semantic-field-control", "Email")
                                .value("person@example.com")
                                .placeholder("Work email")
                                .required(true)
                                .invalid(self.invalid)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .child(
                    Field::new("semantic-textarea-field", "Notes")
                        .help("Keep it concise")
                        .error("Notes are required")
                        .invalid(self.invalid)
                        .control(
                            Textarea::new("semantic-textarea-field-control", "Notes")
                                .value("Ready")
                                .placeholder("Notes prompt")
                                .on_change(|_, _, _| {}),
                        ),
                )
            })
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| FieldProbe {
        invalid: false,
        show: true,
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("field should publish its final accessibility tree");
    let (control_id, control) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::TextInput)
        .map(|(id, node)| (*id, node))
        .expect("field control should be present");
    let (label_id, _) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::Label && node.label() == Some("Email"))
        .map(|(id, node)| (*id, node))
        .expect("field label should be a semantic node");
    let (help_id, _) = a11y_node_with_label(&initial, "Use a work address");
    let (textarea_id, textarea) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::MultilineTextInput)
        .map(|(id, node)| (*id, node))
        .expect("field textarea control should be present");
    let (textarea_label_id, _) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::Label && node.label() == Some("Notes"))
        .map(|(id, node)| (*id, node))
        .expect("textarea field label should be a semantic node");
    let (textarea_help_id, _) = a11y_node_with_label(&initial, "Keep it concise");
    assert_eq!(control.label(), None);
    assert_eq!(control.placeholder(), Some("Work email"));
    assert_eq!(control.labelled_by(), &[label_id]);
    assert_eq!(control.described_by(), &[help_id]);
    assert_eq!(control.error_message(), None);
    assert!(control.is_required());
    assert_eq!(control.invalid(), None);
    assert_eq!(textarea.label(), None);
    assert_eq!(textarea.placeholder(), Some("Notes prompt"));
    assert_eq!(textarea.labelled_by(), &[textarea_label_id]);
    assert_eq!(textarea.described_by(), &[textarea_help_id]);
    assert_eq!(textarea.error_message(), None);

    view.update(cx, |probe, cx| {
        probe.invalid = true;
        cx.notify();
    });
    cx.run_until_parked();

    let invalid = cx
        .latest_accessibility_tree_update()
        .expect("invalid field should publish");
    let invalid_control = a11y_node_by_id(&invalid, control_id);
    let invalid_label = a11y_node_by_id(&invalid, label_id);
    let (error_id, _) = a11y_node_with_label(&invalid, "Enter a valid email");
    let textarea_invalid = a11y_node_by_id(&invalid, textarea_id);
    let (textarea_error_id, _) = a11y_node_with_label(&invalid, "Notes are required");
    assert_eq!(invalid_control.role(), accesskit::Role::TextInput);
    assert_eq!(invalid_label.role(), accesskit::Role::Label);
    assert_eq!(invalid_label.label(), Some("Email"));
    assert_eq!(invalid_control.labelled_by(), &[label_id]);
    assert!(invalid_control.described_by().is_empty());
    assert_eq!(invalid_control.error_message(), Some(error_id));
    assert_eq!(invalid_control.invalid(), Some(accesskit::Invalid::True));
    assert!(!invalid.nodes.iter().any(|(id, _)| *id == help_id));
    assert_eq!(textarea_invalid.labelled_by(), &[textarea_label_id]);
    assert!(textarea_invalid.described_by().is_empty());
    assert_eq!(textarea_invalid.error_message(), Some(textarea_error_id));
    assert!(!invalid.nodes.iter().any(|(id, _)| *id == textarea_help_id));

    view.update(cx, |probe, cx| {
        probe.invalid = false;
        cx.notify();
    });
    cx.run_until_parked();

    let restored = cx
        .latest_accessibility_tree_update()
        .expect("restored field should publish");
    let restored_control = a11y_node_by_id(&restored, control_id);
    let (restored_help_id, _) = a11y_node_with_label(&restored, "Use a work address");
    let restored_textarea = a11y_node_by_id(&restored, textarea_id);
    let (restored_textarea_help_id, _) = a11y_node_with_label(&restored, "Keep it concise");
    assert_eq!(restored_control.role(), accesskit::Role::TextInput);
    assert_eq!(restored_help_id, help_id);
    assert_eq!(restored_control.described_by(), &[help_id]);
    assert_eq!(restored_control.error_message(), None);
    assert!(!restored.nodes.iter().any(|(id, _)| *id == error_id));
    assert_eq!(restored_textarea_help_id, textarea_help_id);
    assert_eq!(restored_textarea.described_by(), &[textarea_help_id]);
    assert_eq!(restored_textarea.error_message(), None);
    assert!(
        !restored
            .nodes
            .iter()
            .any(|(id, _)| *id == textarea_error_id)
    );

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("field unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| matches!(
        *id,
        id if id == control_id
            || id == label_id
            || id == help_id
            || id == textarea_id
            || id == textarea_label_id
            || id == textarea_help_id
    )));
}

#[open_gpui::test]
fn external_field_controls_can_apply_relations_and_explicit_state_overrides(
    cx: &mut open_gpui::TestAppContext,
) {
    struct ExternalFieldProbe {
        field_value: bool,
        child_value: bool,
    }

    impl Render for ExternalFieldProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let child_state = FormControlState::new(Size::Medium)
                .with_required(self.child_value)
                .with_disabled(self.child_value)
                .with_invalid(self.child_value)
                .with_busy(self.child_value);

            Field::new("external-field", "External control")
                .help("External help")
                .error("External error")
                .required(self.field_value)
                .disabled(self.field_value)
                .invalid(self.field_value)
                .busy(self.field_value)
                .control(ExternalFieldControl::new(
                    "external-field-control",
                    child_state,
                ))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| ExternalFieldProbe {
        field_value: true,
        child_value: false,
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("external field control should publish");
    let (control_id, control) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::TextInput)
        .map(|(id, node)| (*id, node))
        .expect("external field control should be present");
    let (label_id, _) = a11y_node_with_label(&initial, "External control");
    let (error_id, _) = a11y_node_with_label(&initial, "External error");
    assert_eq!(control.labelled_by(), &[label_id]);
    assert!(control.described_by().is_empty());
    assert_eq!(control.error_message(), Some(error_id));
    assert!(control.is_required());
    assert!(control.is_disabled());
    assert!(control.is_busy());
    assert_eq!(control.invalid(), Some(accesskit::Invalid::True));

    view.update(cx, |probe, cx| {
        probe.field_value = false;
        probe.child_value = true;
        cx.notify();
    });
    cx.run_until_parked();

    let updated = cx
        .latest_accessibility_tree_update()
        .expect("external field overrides should update");
    let updated_control = a11y_node_by_id(&updated, control_id);
    let (help_id, _) = a11y_node_with_label(&updated, "External help");
    assert_eq!(updated_control.labelled_by(), &[label_id]);
    assert_eq!(updated_control.described_by(), &[help_id]);
    assert_eq!(updated_control.error_message(), None);
    assert!(!updated_control.is_required());
    assert!(!updated_control.is_disabled());
    assert!(!updated_control.is_busy());
    assert_eq!(updated_control.invalid(), None);
}

#[open_gpui::test]
fn field_explicit_shared_state_overrides_controls_without_default_state_erasure(
    cx: &mut open_gpui::TestAppContext,
) {
    struct FieldAuthorityProbe {
        field_override: Option<bool>,
        child_flags: bool,
        input_value: Rc<RefCell<String>>,
        textarea_value: Rc<RefCell<String>>,
        input_changes: Rc<RefCell<Vec<String>>>,
        textarea_changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for FieldAuthorityProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let input_value = self.input_value.borrow().clone();
            let next_input_value = self.input_value.clone();
            let input_changes = self.input_changes.clone();
            let textarea_value = self.textarea_value.borrow().clone();
            let next_textarea_value = self.textarea_value.clone();
            let textarea_changes = self.textarea_changes.clone();

            let mut input_field = Field::new("authority-input-field", "Authority input").control(
                TextInput::new("authority-input-control", "Authority input")
                    .value(input_value)
                    .required(self.child_flags)
                    .invalid(self.child_flags)
                    .busy(self.child_flags)
                    .disabled(self.child_flags)
                    .on_change(move |value, _, _| {
                        *next_input_value.borrow_mut() = value.clone();
                        input_changes.borrow_mut().push(value);
                    }),
            );
            let mut textarea_field = Field::new("authority-textarea-field", "Authority textarea")
                .control(
                    Textarea::new("authority-textarea-control", "Authority textarea")
                        .value(textarea_value)
                        .required(self.child_flags)
                        .invalid(self.child_flags)
                        .busy(self.child_flags)
                        .disabled(self.child_flags)
                        .on_change(move |value, _, _| {
                            *next_textarea_value.borrow_mut() = value.clone();
                            textarea_changes.borrow_mut().push(value);
                        }),
                );
            if let Some(value) = self.field_override {
                input_field = input_field
                    .required(value)
                    .invalid(value)
                    .busy(value)
                    .disabled(value);
                textarea_field = textarea_field
                    .required(value)
                    .invalid(value)
                    .busy(value)
                    .disabled(value);
            }

            div().child(input_field).child(textarea_field)
        }
    }

    let input_value = Rc::new(RefCell::new("input".to_owned()));
    let textarea_value = Rc::new(RefCell::new("textarea".to_owned()));
    let input_changes = Rc::new(RefCell::new(Vec::new()));
    let textarea_changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| FieldAuthorityProbe {
        field_override: None,
        child_flags: true,
        input_value,
        textarea_value,
        input_changes: input_changes.clone(),
        textarea_changes: textarea_changes.clone(),
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("field-owned controls should publish");
    let (input_id, input) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::TextInput)
        .map(|(id, node)| (*id, node))
        .expect("input control should publish");
    let (textarea_id, textarea) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::MultilineTextInput)
        .map(|(id, node)| (*id, node))
        .expect("textarea control should publish");
    for control in [input, textarea] {
        assert!(control.is_required());
        assert_eq!(control.invalid(), Some(accesskit::Invalid::True));
        assert!(control.is_busy());
        assert!(control.is_disabled());
        assert_exact_actions(control, &[]);
    }

    view.update(cx, |probe, cx| {
        probe.field_override = Some(false);
        cx.notify();
    });
    cx.run_until_parked();

    let explicitly_enabled = cx
        .latest_accessibility_tree_update()
        .expect("explicit false field state should publish");
    for (id, role) in [
        (input_id, accesskit::Role::TextInput),
        (textarea_id, accesskit::Role::MultilineTextInput),
    ] {
        let control = a11y_node_by_id(&explicitly_enabled, id);
        assert_eq!(control.role(), role);
        assert!(!control.is_required());
        assert_eq!(control.invalid(), None);
        assert!(!control.is_busy());
        assert!(!control.is_disabled());
        assert_exact_actions(
            control,
            &[
                accesskit::Action::Focus,
                accesskit::Action::ReplaceSelectedText,
                accesskit::Action::SetTextSelection,
                accesskit::Action::SetValue,
            ],
        );
    }
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value("input enabled".into())),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        textarea_id,
        Some(accesskit::ActionData::Value("textarea enabled".into())),
    )));
    assert_eq!(input_changes.borrow().as_slice(), ["input enabled"]);
    assert_eq!(textarea_changes.borrow().as_slice(), ["textarea enabled"]);

    view.update(cx, |probe, cx| {
        probe.field_override = Some(true);
        probe.child_flags = false;
        cx.notify();
    });
    cx.run_until_parked();

    let explicitly_disabled = cx
        .latest_accessibility_tree_update()
        .expect("explicit true field state should publish");
    for id in [input_id, textarea_id] {
        let control = a11y_node_by_id(&explicitly_disabled, id);
        assert!(control.is_required());
        assert_eq!(control.invalid(), Some(accesskit::Invalid::True));
        assert!(control.is_busy());
        assert!(control.is_disabled());
        assert_exact_actions(control, &[]);
        assert!(cx.dispatch_accessibility_action(action_request(
            accesskit::Action::SetValue,
            id,
            Some(accesskit::ActionData::Value("blocked".into())),
        )));
    }
    assert_eq!(input_changes.borrow().as_slice(), ["input enabled"]);
    assert_eq!(textarea_changes.borrow().as_slice(), ["textarea enabled"]);
}
