use super::common::*;

#[open_gpui::test]
fn accessible_replacement_targets_the_selection_during_ime_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    struct CompositionProbe {
        input_value: Rc<RefCell<String>>,
        textarea_value: Rc<RefCell<String>>,
        input_changes: Rc<RefCell<Vec<String>>>,
        textarea_changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for CompositionProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let input_value = self.input_value.borrow().clone();
            let next_input_value = self.input_value.clone();
            let input_changes = self.input_changes.clone();
            let textarea_value = self.textarea_value.borrow().clone();
            let next_textarea_value = self.textarea_value.clone();
            let textarea_changes = self.textarea_changes.clone();

            div()
                .child(
                    TextInput::new("composing-text-input", "Composing account")
                        .value(input_value)
                        .on_change(move |value, _, _| {
                            *next_input_value.borrow_mut() = value.clone();
                            input_changes.borrow_mut().push(value);
                        }),
                )
                .child(
                    Textarea::new("composing-textarea", "Composing notes")
                        .value(textarea_value)
                        .on_change(move |value, _, _| {
                            *next_textarea_value.borrow_mut() = value.clone();
                            textarea_changes.borrow_mut().push(value);
                        }),
                )
        }
    }

    let input_value = Rc::new(RefCell::new(String::new()));
    let textarea_value = Rc::new(RefCell::new(String::new()));
    let input_changes = Rc::new(RefCell::new(Vec::new()));
    let textarea_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| CompositionProbe {
        input_value,
        textarea_value,
        input_changes: input_changes.clone(),
        textarea_changes: textarea_changes.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("composition controls should publish");
    let (input_id, _) = a11y_node_with_label(&initial, "Composing account");
    let (textarea_id, _) = a11y_node_with_label(&initial, "Composing notes");

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        input_id,
        None,
    )));
    cx.simulate_marked_text(None, "ni", Some(1..2));
    assert_eq!(input_changes.borrow().as_slice(), ["ni"]);
    let input_composing = cx
        .latest_accessibility_tree_update()
        .expect("text input composition should publish");
    let input_control = a11y_node_by_id(&input_composing, input_id);
    let (input_text_run_id, input_text_run) = a11y_text_run_child(&input_composing, input_control);
    assert_eq!(input_text_run.value(), Some("ni"));
    assert_eq!(input_text_run.character_lengths(), &[1, 1]);
    assert_eq!(
        input_control.text_selection(),
        Some(&accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: input_text_run_id,
                character_index: 1,
            },
            focus: accesskit::TextPosition {
                node: input_text_run_id,
                character_index: 2,
            },
        })
    );
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
                    character_index: 1,
                },
            },
        )),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        input_id,
        Some(accesskit::ActionData::Value("!".into())),
    )));
    assert_eq!(input_changes.borrow().as_slice(), ["ni", "!i"]);
    assert_eq!(
        {
            let update = cx
                .latest_accessibility_tree_update()
                .expect("text input replacement should publish");
            let control = a11y_node_by_id(&update, input_id);
            let (text_run_id, text_run) = a11y_text_run_child(&update, control);
            assert_eq!(text_run_id, input_text_run_id);
            text_run.value().map(ToOwned::to_owned)
        },
        Some("!i".to_owned())
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        textarea_id,
        None,
    )));
    cx.simulate_marked_text(None, "ni", Some(1..2));
    assert_eq!(textarea_changes.borrow().as_slice(), ["ni"]);
    let textarea_composing = cx
        .latest_accessibility_tree_update()
        .expect("textarea composition should publish");
    let textarea_control = a11y_node_by_id(&textarea_composing, textarea_id);
    let (textarea_text_run_id, textarea_text_run) =
        a11y_text_run_child(&textarea_composing, textarea_control);
    assert_eq!(textarea_text_run.value(), Some("ni"));
    assert_eq!(textarea_text_run.character_lengths(), &[1, 1]);
    assert_eq!(
        textarea_control.text_selection(),
        Some(&accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: textarea_text_run_id,
                character_index: 1,
            },
            focus: accesskit::TextPosition {
                node: textarea_text_run_id,
                character_index: 2,
            },
        })
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
                    character_index: 1,
                },
            },
        )),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        textarea_id,
        Some(accesskit::ActionData::Value("!".into())),
    )));
    assert_eq!(textarea_changes.borrow().as_slice(), ["ni", "!i"]);
    assert_eq!(
        {
            let update = cx
                .latest_accessibility_tree_update()
                .expect("textarea replacement should publish");
            let control = a11y_node_by_id(&update, textarea_id);
            let (text_run_id, text_run) = a11y_text_run_child(&update, control);
            assert_eq!(text_run_id, textarea_text_run_id);
            text_run.value().map(ToOwned::to_owned)
        },
        Some("!i".to_owned())
    );
}

#[open_gpui::test]
fn ime_selection_inside_a_grapheme_remains_observable_in_the_final_tree(
    cx: &mut open_gpui::TestAppContext,
) {
    struct ImeGraphemeProbe {
        value: Rc<RefCell<String>>,
    }

    impl Render for ImeGraphemeProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            TextInput::new("ime-grapheme-input", "IME grapheme")
                .value(value)
                .on_change(move |value, _, _| *next_value.borrow_mut() = value)
        }
    }

    let value = Rc::new(RefCell::new(String::new()));
    let (_, cx) = cx.add_window_view(|_, _| ImeGraphemeProbe {
        value: value.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("IME input should publish");
    let (input_id, _) = a11y_node_with_label(&initial, "IME grapheme");
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        input_id,
        None,
    )));

    cx.simulate_marked_text(None, "e\u{301}", Some(1..1));
    let combining = cx
        .latest_accessibility_tree_update()
        .expect("combining composition should publish");
    let combining_input = a11y_node_by_id(&combining, input_id);
    let (text_run_id, combining_run) = a11y_text_run_child(&combining, combining_input);
    assert_eq!(combining_run.character_lengths(), &["e\u{301}".len() as u8]);
    let grapheme_start = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_id,
            character_index: 0,
        },
        focus: accesskit::TextPosition {
            node: text_run_id,
            character_index: 0,
        },
    };
    assert_eq!(combining_input.text_selection(), Some(&grapheme_start));

    const FAMILY: &str = "👨‍👩‍👧‍👦";
    cx.simulate_marked_text(None, FAMILY, Some(2..9));
    let zwj = cx
        .latest_accessibility_tree_update()
        .expect("ZWJ composition should publish");
    let zwj_input = a11y_node_by_id(&zwj, input_id);
    let (zwj_text_run_id, zwj_run) = a11y_text_run_child(&zwj, zwj_input);
    assert_eq!(zwj_text_run_id, text_run_id);
    assert_eq!(zwj_run.value(), Some(FAMILY));
    assert_eq!(zwj_run.character_lengths(), &[FAMILY.len() as u8]);
    assert_eq!(zwj_input.text_selection(), Some(&grapheme_start));
    assert_eq!(value.borrow().as_str(), FAMILY);
}

#[open_gpui::test]
fn rejected_ime_composition_preserves_the_platform_input_handler(
    cx: &mut open_gpui::TestAppContext,
) {
    struct ReadOnlyCompositionProbe {
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for ReadOnlyCompositionProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            TextInput::new("read-only-composition", "Read-only composition")
                .value("stable")
                .read_only(true)
                .on_change(move |value, _, _| changes.borrow_mut().push(value))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| ReadOnlyCompositionProbe {
        changes: changes.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("read-only input should publish");
    let (input_id, _) = a11y_node_with_label(&initial, "Read-only composition");
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::Focus,
        input_id,
        None,
    )));
    cx.run_until_parked();
    cx.skip_drawing();

    cx.simulate_marked_text(None, "first", None);
    cx.simulate_marked_text(None, "second", None);

    assert!(changes.borrow().is_empty());
    let unchanged = cx
        .latest_accessibility_tree_update()
        .expect("rejected composition should retain the published tree");
    assert_eq!(
        a11y_node_by_id(&unchanged, input_id).value(),
        Some("stable")
    );
}
