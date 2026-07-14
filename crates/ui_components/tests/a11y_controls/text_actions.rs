use super::common::*;

#[open_gpui::test]
fn text_fields_project_final_semantics_and_dispatch_text_actions(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TextFieldsProbe {
        input_value: Rc<RefCell<String>>,
        textarea_value: Rc<RefCell<String>>,
        input_changes: Rc<RefCell<Vec<String>>>,
        textarea_changes: Rc<RefCell<Vec<String>>>,
        input_disabled: bool,
        input_read_only: bool,
        textarea_disabled: bool,
        textarea_read_only: bool,
        semantic_flags: bool,
        show: bool,
    }

    impl Render for TextFieldsProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let input_value = self.input_value.borrow().clone();
            let next_input_value = self.input_value.clone();
            let input_changes = self.input_changes.clone();
            let textarea_value = self.textarea_value.borrow().clone();
            let next_textarea_value = self.textarea_value.clone();
            let textarea_changes = self.textarea_changes.clone();

            div().when(self.show, |this| {
                this.child(
                    TextInput::new("semantic-text-input", "Account name")
                        .value(input_value)
                        .required(self.semantic_flags)
                        .invalid(self.semantic_flags)
                        .busy(self.semantic_flags)
                        .disabled(self.input_disabled)
                        .read_only(self.input_read_only)
                        .on_change(move |value, _, _| {
                            *next_input_value.borrow_mut() = value.clone();
                            input_changes.borrow_mut().push(value);
                        }),
                )
                .child(
                    Textarea::new("semantic-textarea", "Release notes")
                        .value(textarea_value)
                        .required(self.semantic_flags)
                        .invalid(self.semantic_flags)
                        .busy(self.semantic_flags)
                        .disabled(self.textarea_disabled)
                        .read_only(self.textarea_read_only)
                        .on_change(move |value, _, _| {
                            *next_textarea_value.borrow_mut() = value.clone();
                            textarea_changes.borrow_mut().push(value);
                        }),
                )
                .child(
                    TextInput::new("semantic-static-text-input", "Static account")
                        .value("readable"),
                )
                .child(
                    Textarea::new("semantic-static-textarea", "Static notes")
                        .value("readable\nnotes"),
                )
            })
        }
    }

    let input_value = Rc::new(RefCell::new("alpha".to_owned()));
    let textarea_value = Rc::new(RefCell::new("first line".to_owned()));
    let input_changes = Rc::new(RefCell::new(Vec::new()));
    let textarea_changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TextFieldsProbe {
        input_value: input_value.clone(),
        textarea_value: textarea_value.clone(),
        input_changes: input_changes.clone(),
        textarea_changes: textarea_changes.clone(),
        input_disabled: false,
        input_read_only: false,
        textarea_disabled: false,
        textarea_read_only: false,
        semantic_flags: true,
        show: true,
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("text fields should publish their final accessibility tree");
    let (input_id, input) = a11y_node_with_label(&initial, "Account name");
    assert_eq!(input.role(), accesskit::Role::TextInput);
    assert_eq!(input.value(), Some("alpha"));
    assert_eq!(input.invalid(), Some(accesskit::Invalid::True));
    assert!(input.is_required());
    assert!(input.is_busy());
    assert!(!input.is_read_only());
    assert!(!input.is_disabled());
    assert_exact_actions(
        input,
        &[
            accesskit::Action::Focus,
            accesskit::Action::ReplaceSelectedText,
            accesskit::Action::SetTextSelection,
            accesskit::Action::SetValue,
        ],
    );
    let (input_text_run_id, _) = a11y_text_run_child(&initial, input);

    let (textarea_id, textarea) = a11y_node_with_label(&initial, "Release notes");
    assert_eq!(textarea.role(), accesskit::Role::MultilineTextInput);
    assert_eq!(textarea.value(), Some("first line"));
    assert_eq!(textarea.invalid(), Some(accesskit::Invalid::True));
    assert!(textarea.is_required());
    assert!(textarea.is_busy());
    assert_exact_actions(
        textarea,
        &[
            accesskit::Action::Focus,
            accesskit::Action::ReplaceSelectedText,
            accesskit::Action::SetTextSelection,
            accesskit::Action::SetValue,
        ],
    );
    let (textarea_text_run_id, _) = a11y_text_run_child(&initial, textarea);

    let (static_input_id, static_input) = a11y_node_with_label(&initial, "Static account");
    assert_eq!(static_input.role(), accesskit::Role::TextInput);
    assert_eq!(static_input.value(), Some("readable"));
    assert_exact_actions(static_input, &[accesskit::Action::Focus]);

    let (static_textarea_id, static_textarea) = a11y_node_with_label(&initial, "Static notes");
    assert_eq!(static_textarea.role(), accesskit::Role::MultilineTextInput);
    assert_eq!(static_textarea.value(), Some("readable\nnotes"));
    assert_exact_actions(static_textarea, &[accesskit::Action::Focus]);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value("beta\nline".into())),
    )));
    cx.run_until_parked();
    let input_set_value_update = cx
        .latest_accessibility_tree_update()
        .expect("text input SetValue should publish");
    assert_eq!(
        a11y_node_by_id(&input_set_value_update, input_id).value(),
        Some("beta line")
    );
    assert_eq!(input_changes.borrow().as_slice(), ["beta line"]);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        input_id,
        Some(accesskit::ActionData::Value("!".into())),
    )));
    cx.run_until_parked();
    let input_replacement_update = cx
        .latest_accessibility_tree_update()
        .expect("text input replacement should publish");
    assert_eq!(
        a11y_node_by_id(&input_replacement_update, input_id).value(),
        Some("beta line!")
    );
    assert_eq!(
        input_changes.borrow().as_slice(),
        ["beta line", "beta line!"]
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        textarea_id,
        Some(accesskit::ActionData::Value("line 1\r\nline 2".into())),
    )));
    cx.run_until_parked();
    let textarea_set_value_update = cx
        .latest_accessibility_tree_update()
        .expect("textarea SetValue should publish");
    assert_eq!(
        a11y_node_by_id(&textarea_set_value_update, textarea_id).value(),
        Some("line 1\nline 2")
    );
    assert_eq!(textarea_changes.borrow().as_slice(), ["line 1\nline 2"]);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        textarea_id,
        Some(accesskit::ActionData::Value("!".into())),
    )));
    cx.run_until_parked();
    let textarea_replacement_update = cx
        .latest_accessibility_tree_update()
        .expect("textarea replacement should publish");
    assert_eq!(
        a11y_node_by_id(&textarea_replacement_update, textarea_id).value(),
        Some("line 1\nline 2!")
    );
    assert_eq!(
        textarea_changes.borrow().as_slice(),
        ["line 1\nline 2", "line 1\nline 2!"]
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::NumericValue(7.0)),
    )));
    assert_eq!(input_changes.borrow().len(), 2);

    view.update(cx, |probe, cx| {
        probe.input_read_only = true;
        probe.textarea_disabled = true;
        probe.semantic_flags = false;
        cx.notify();
    });
    cx.run_until_parked();

    let restricted = cx
        .latest_accessibility_tree_update()
        .expect("restricted text fields should publish");
    let (restricted_input_id, restricted_input) = a11y_node_with_label(&restricted, "Account name");
    assert_eq!(restricted_input_id, input_id);
    assert_eq!(restricted_input.value(), Some("beta line!"));
    assert!(restricted_input.is_read_only());
    assert_eq!(restricted_input.invalid(), None);
    assert!(!restricted_input.is_required());
    assert!(!restricted_input.is_busy());
    assert_exact_actions(
        restricted_input,
        &[
            accesskit::Action::Focus,
            accesskit::Action::SetTextSelection,
        ],
    );

    let (restricted_textarea_id, restricted_textarea) =
        a11y_node_with_label(&restricted, "Release notes");
    assert_eq!(restricted_textarea_id, textarea_id);
    assert_eq!(restricted_textarea.value(), Some("line 1\nline 2!"));
    assert!(restricted_textarea.is_disabled());
    assert_eq!(restricted_textarea.invalid(), None);
    assert!(!restricted_textarea.is_required());
    assert!(!restricted_textarea.is_busy());
    assert_exact_actions(restricted_textarea, &[]);

    let restricted_input_selection = *restricted_input
        .text_selection()
        .expect("read-only input should expose its selection");
    let restricted_textarea_selection = *restricted_textarea
        .text_selection()
        .expect("disabled textarea should retain observable selection state");
    assert_eq!(
        a11y_text_run_child(&restricted, restricted_input).0,
        input_text_run_id
    );
    assert_eq!(
        a11y_text_run_child(&restricted, restricted_textarea).0,
        textarea_text_run_id
    );
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        textarea_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: textarea_text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: textarea_text_run_id,
                    character_index: 0,
                },
            },
        )),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: input_text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: input_text_run_id,
                    character_index: 0,
                },
            },
        )),
    )));
    cx.run_until_parked();
    let restricted_selection_update = cx
        .latest_accessibility_tree_update()
        .expect("read-only selection should publish");
    assert_eq!(
        a11y_node_by_id(&restricted_selection_update, input_id).text_selection(),
        Some(&accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: input_text_run_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: input_text_run_id,
                character_index: 0,
            },
        })
    );
    assert_eq!(
        a11y_node_by_id(&restricted_selection_update, textarea_id).text_selection(),
        Some(&restricted_textarea_selection)
    );
    assert_ne!(restricted_input_selection.focus.character_index, 0);

    assert_eq!(
        a11y_node_by_id(&restricted, static_input_id).value(),
        Some("readable")
    );
    assert_eq!(
        a11y_node_by_id(&restricted, static_textarea_id).value(),
        Some("readable\nnotes")
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value("blocked".into())),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        textarea_id,
        Some(accesskit::ActionData::Value("blocked".into())),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        input_id,
        Some(accesskit::ActionData::Value("blocked".into())),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        textarea_id,
        Some(accesskit::ActionData::Value("blocked".into())),
    )));
    assert_eq!(input_changes.borrow().len(), 2);
    assert_eq!(textarea_changes.borrow().len(), 2);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        input_id,
        None,
    )));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("text input focus should publish")
            .focus,
        input_id
    );

    view.update(cx, |probe, cx| {
        probe.input_disabled = true;
        probe.input_read_only = false;
        probe.textarea_disabled = false;
        probe.textarea_read_only = true;
        cx.notify();
    });
    cx.run_until_parked();

    let reverse_restricted = cx
        .latest_accessibility_tree_update()
        .expect("reverse restricted text fields should publish");
    let reverse_input = a11y_node_by_id(&reverse_restricted, input_id);
    assert!(reverse_input.is_disabled());
    assert!(!reverse_input.is_read_only());
    assert_exact_actions(reverse_input, &[]);
    let reverse_textarea = a11y_node_by_id(&reverse_restricted, textarea_id);
    assert!(!reverse_textarea.is_disabled());
    assert!(reverse_textarea.is_read_only());
    assert_exact_actions(
        reverse_textarea,
        &[
            accesskit::Action::Focus,
            accesskit::Action::SetTextSelection,
        ],
    );

    let reverse_input_selection = *reverse_input
        .text_selection()
        .expect("disabled input should retain observable selection state");
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: input_text_run_id,
                    character_index: 1,
                },
                focus: accesskit::TextPosition {
                    node: input_text_run_id,
                    character_index: 1,
                },
            },
        )),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        textarea_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: textarea_text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: textarea_text_run_id,
                    character_index: 0,
                },
            },
        )),
    )));
    cx.run_until_parked();
    let reverse_selection_update = cx
        .latest_accessibility_tree_update()
        .expect("reverse restriction selection should publish");
    assert_eq!(
        a11y_node_by_id(&reverse_selection_update, input_id).text_selection(),
        Some(&reverse_input_selection)
    );
    assert_eq!(
        a11y_node_by_id(&reverse_selection_update, textarea_id).text_selection(),
        Some(&accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: textarea_text_run_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: textarea_text_run_id,
                character_index: 0,
            },
        })
    );

    for (node_id, action) in [
        (input_id, accesskit::Action::SetValue),
        (input_id, accesskit::Action::ReplaceSelectedText),
        (textarea_id, accesskit::Action::SetValue),
        (textarea_id, accesskit::Action::ReplaceSelectedText),
    ] {
        assert!(cx.dispatch_accessibility_action(action_request(
            action,
            node_id,
            Some(accesskit::ActionData::Value("still blocked".into())),
        )));
    }
    assert_eq!(input_changes.borrow().len(), 2);
    assert_eq!(textarea_changes.borrow().len(), 2);
    let unchanged = cx
        .latest_accessibility_tree_update()
        .expect("blocked actions must leave the final tree unchanged");
    assert_eq!(
        a11y_node_by_id(&unchanged, input_id).value(),
        Some("beta line!")
    );
    assert_eq!(
        a11y_node_by_id(&unchanged, textarea_id).value(),
        Some("line 1\nline 2!")
    );

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("text field unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| matches!(
        *id,
        id if id == input_id
            || id == textarea_id
            || id == static_input_id
            || id == static_textarea_id
    )));
}

#[open_gpui::test]
fn text_field_placeholders_follow_the_resolved_render_state(cx: &mut open_gpui::TestAppContext) {
    struct PlaceholderProbe {
        expose_placeholders: bool,
    }

    impl Render for PlaceholderProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let input = TextInput::new("placeholder-text-input", "Empty account")
                .on_change(|_, _, _| {})
                .when(self.expose_placeholders, |input| {
                    input.placeholder("Email address")
                });
            let textarea = Textarea::new("placeholder-textarea", "Empty notes")
                .on_change(|_, _, _| {})
                .when(self.expose_placeholders, |textarea| {
                    textarea.placeholder("Write release notes")
                });

            div().child(input).child(textarea)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| PlaceholderProbe {
        expose_placeholders: true,
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("placeholder controls should publish");
    let (input_id, input) = a11y_node_with_label(&initial, "Empty account");
    let (textarea_id, textarea) = a11y_node_with_label(&initial, "Empty notes");
    assert_eq!(input.value(), Some(""));
    assert_eq!(input.placeholder(), Some("Email address"));
    assert_eq!(textarea.value(), Some(""));
    assert_eq!(textarea.placeholder(), Some("Write release notes"));

    view.update(cx, |probe, cx| {
        probe.expose_placeholders = false;
        cx.notify();
    });
    cx.run_until_parked();

    let cleared = cx
        .latest_accessibility_tree_update()
        .expect("cleared placeholders should publish");
    assert_eq!(a11y_node_by_id(&cleared, input_id).placeholder(), None);
    assert_eq!(a11y_node_by_id(&cleared, textarea_id).placeholder(), None);
}

#[open_gpui::test]
fn external_text_input_controller_owns_final_tree_and_action_lifecycle(
    cx: &mut open_gpui::TestAppContext,
) {
    struct ExternalControllerProbe {
        controller: open_gpui::Entity<TextInputController>,
        show: bool,
    }

    impl Render for ExternalControllerProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().children(self.show.then(|| {
                TextInput::new("external-controller-input", "External controller")
                    .controller(self.controller.clone())
            }))
        }
    }

    cx.update(init_text_input);
    let controller = cx.new(TextInputController::new);
    cx.update_entity(&controller, |controller, cx| {
        controller.set_value("ab", cx);
        controller.set_placeholder("Controller placeholder", cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| ExternalControllerProbe {
        controller: controller.clone(),
        show: true,
    });
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("external controller should publish");
    let (input_id, input) = a11y_node_with_label(&initial, "External controller");
    let (text_run_id, initial_text_run) = a11y_text_run_child(&initial, input);
    assert_eq!(initial_text_run.value(), Some("ab"));
    assert_eq!(input.placeholder(), Some("Controller placeholder"));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value("e\u{301}".into())),
    )));
    cx.run_until_parked();
    let valued = cx
        .latest_accessibility_tree_update()
        .expect("external controller value should publish");
    let valued_input = a11y_node_by_id(&valued, input_id);
    let (valued_text_run_id, valued_text_run) = a11y_text_run_child(&valued, valued_input);
    assert_eq!(valued_text_run_id, text_run_id);
    assert_eq!(valued_text_run.value(), Some("e\u{301}"));
    assert_eq!(
        valued_text_run.character_lengths(),
        &["e\u{301}".len() as u8]
    );
    assert_eq!(valued_input.placeholder(), Some("Controller placeholder"));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 1,
                },
            },
        )),
    )));
    cx.update_entity(&controller, |controller, cx| {
        assert_eq!(controller.selected_range(), 0.."e\u{301}".len());
        controller.set_placeholder("Updated controller placeholder", cx);
    });
    cx.run_until_parked();
    let updated = cx
        .latest_accessibility_tree_update()
        .expect("external controller placeholder should update");
    assert_eq!(
        a11y_node_by_id(&updated, input_id).placeholder(),
        Some("Updated controller placeholder")
    );

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("external controller unmount should publish");
    assert!(
        !unmounted
            .nodes
            .iter()
            .any(|(node_id, _)| { *node_id == input_id || *node_id == text_run_id })
    );
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value("stale".into())),
    )));
    assert_eq!(
        controller.read_with(cx, |controller, _| controller.value().to_owned()),
        "e\u{301}"
    );
}
