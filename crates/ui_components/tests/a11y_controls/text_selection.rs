use super::common::*;

#[open_gpui::test]
fn text_selection_uses_grapheme_indices_and_rejects_invalid_targets(
    cx: &mut open_gpui::TestAppContext,
) {
    const FAMILY: &str = "👨‍👩‍👧‍👦";
    const COMBINING: &str = "e\u{301}";

    struct GraphemeSelectionProbe {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for GraphemeSelectionProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();
            TextInput::new("grapheme-selection-input", "Grapheme selection")
                .value(value)
                .on_change(move |value, _, _| {
                    *next_value.borrow_mut() = value.clone();
                    changes.borrow_mut().push(value);
                })
        }
    }

    let value = Rc::new(RefCell::new(format!("a{FAMILY}{COMBINING}中")));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| GraphemeSelectionProbe {
        value,
        changes: changes.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("grapheme input should publish");
    let (input_id, input) = a11y_node_with_label(&initial, "Grapheme selection");
    let (text_run_id, text_run) = a11y_text_run_child(&initial, input);
    assert_eq!(
        text_run.character_lengths(),
        &[
            1,
            FAMILY.len() as u8,
            COMBINING.len() as u8,
            "中".len() as u8,
        ]
    );
    let initial_selection = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_id,
            character_index: 4,
        },
        focus: accesskit::TextPosition {
            node: text_run_id,
            character_index: 4,
        },
    };
    assert_eq!(input.text_selection(), Some(&initial_selection));

    for selection in [
        accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: input_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: input_id,
                character_index: 1,
            },
        },
        accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: text_run_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: text_run_id,
                character_index: 5,
            },
        },
    ] {
        assert!(cx.dispatch_accessibility_action(action_request(
            accesskit::Action::SetTextSelection,
            input_id,
            Some(accesskit::ActionData::SetTextSelection(selection)),
        )));
    }
    cx.run_until_parked();
    let unchanged = cx
        .latest_accessibility_tree_update()
        .expect("invalid selections must leave the tree unchanged");
    assert_eq!(
        a11y_node_by_id(&unchanged, input_id).text_selection(),
        Some(&initial_selection)
    );

    let reversed_selection = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_id,
            character_index: 3,
        },
        focus: accesskit::TextPosition {
            node: text_run_id,
            character_index: 1,
        },
    };
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(reversed_selection,)),
    )));
    cx.run_until_parked();
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("reversed selection should publish");
    assert_eq!(
        a11y_node_by_id(&selected, input_id).text_selection(),
        Some(&reversed_selection)
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        input_id,
        Some(accesskit::ActionData::Value("X".into())),
    )));
    assert_eq!(changes.borrow().as_slice(), ["aX中"]);
    cx.run_until_parked();
    let replaced = cx
        .latest_accessibility_tree_update()
        .expect("grapheme replacement should publish");
    let replaced_input = a11y_node_by_id(&replaced, input_id);
    let (replaced_text_run_id, replaced_text_run) = a11y_text_run_child(&replaced, replaced_input);
    assert_eq!(replaced_text_run_id, text_run_id);
    assert_eq!(replaced_text_run.value(), Some("aX中"));

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 3,
                },
                focus: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 3,
                },
            },
        )),
    )));
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        input_id,
        Some(accesskit::ActionData::Value("!".into())),
    )));
    assert_eq!(changes.borrow().as_slice(), ["aX中", "aX中!"]);
}

#[open_gpui::test]
fn textarea_projects_visual_lines_and_maps_cross_line_accessible_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    const EMOJI: &str = "🙂";

    struct MultilineSelectionProbe {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for MultilineSelectionProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();
            Textarea::new("multiline-selection-textarea", "Multiline selection")
                .value(value)
                .on_change(move |value, _, _| {
                    *next_value.borrow_mut() = value.clone();
                    changes.borrow_mut().push(value);
                })
        }
    }

    let value = Rc::new(RefCell::new(format!("a\n{EMOJI}\n")));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| MultilineSelectionProbe {
        value,
        changes: changes.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("multiline textarea should publish");
    let (textarea_id, textarea) = a11y_node_with_label(&initial, "Multiline selection");
    let text_runs = a11y_text_run_children(&initial, textarea);
    assert_eq!(text_runs.len(), 3);
    assert_eq!(
        text_runs
            .iter()
            .map(|(_, node)| node.value().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["a\n", concat!("🙂", "\n"), ""]
    );
    assert_eq!(text_runs[0].1.character_lengths(), &[1, 1]);
    assert_eq!(text_runs[1].1.character_lengths(), &[EMOJI.len() as u8, 1]);
    assert!(text_runs[2].1.character_lengths().is_empty());
    assert!(
        text_runs.iter().all(|(_, node)| {
            node.previous_on_line().is_none() && node.next_on_line().is_none()
        })
    );
    for (_, text_run) in &text_runs {
        assert_eq!(text_run.previous_on_line(), None);
        assert_eq!(text_run.next_on_line(), None);
    }

    let text_run_ids = text_runs
        .iter()
        .map(|(node_id, _)| *node_id)
        .collect::<Vec<_>>();
    let trailing_caret = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_ids[2],
            character_index: 0,
        },
        focus: accesskit::TextPosition {
            node: text_run_ids[2],
            character_index: 0,
        },
    };
    assert_eq!(textarea.text_selection(), Some(&trailing_caret));

    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let rerendered = cx
        .latest_accessibility_tree_update()
        .expect("equivalent textarea rerender should publish");
    let (_, rerendered_textarea) = a11y_node_with_label(&rerendered, "Multiline selection");
    assert_eq!(
        a11y_text_run_children(&rerendered, rerendered_textarea)
            .iter()
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>(),
        text_run_ids
    );

    let previous_line_end = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_ids[0],
            character_index: 2,
        },
        focus: accesskit::TextPosition {
            node: text_run_ids[0],
            character_index: 2,
        },
    };
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        textarea_id,
        Some(accesskit::ActionData::SetTextSelection(previous_line_end)),
    )));
    cx.run_until_parked();
    let line_boundary = cx
        .latest_accessibility_tree_update()
        .expect("line-boundary selection should publish");
    assert_eq!(
        a11y_node_by_id(&line_boundary, textarea_id).text_selection(),
        Some(&accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: text_run_ids[1],
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: text_run_ids[1],
                character_index: 0,
            },
        })
    );

    let reversed_cross_line = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_ids[1],
            character_index: 1,
        },
        focus: accesskit::TextPosition {
            node: text_run_ids[0],
            character_index: 1,
        },
    };
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        textarea_id,
        Some(accesskit::ActionData::SetTextSelection(reversed_cross_line,)),
    )));
    cx.run_until_parked();
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("cross-line selection should publish");
    assert_eq!(
        a11y_node_by_id(&selected, textarea_id).text_selection(),
        Some(&reversed_cross_line)
    );

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::ReplaceSelectedText,
        textarea_id,
        Some(accesskit::ActionData::Value("X".into())),
    )));
    assert_eq!(changes.borrow().as_slice(), ["aX\n"]);
    cx.run_until_parked();
    let replaced = cx
        .latest_accessibility_tree_update()
        .expect("cross-line replacement should publish");
    let replaced_textarea = a11y_node_by_id(&replaced, textarea_id);
    let replaced_runs = a11y_text_run_children(&replaced, replaced_textarea);
    assert_eq!(
        replaced_runs
            .iter()
            .map(|(_, node)| node.value().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["aX\n", ""]
    );
    assert_eq!(replaced_runs[0].0, text_run_ids[0]);
    assert_eq!(replaced_runs[1].0, text_run_ids[1]);
    assert!(!replaced.nodes.iter().any(|(id, _)| *id == text_run_ids[2]));
    let replaced_selection = *replaced_textarea
        .text_selection()
        .expect("replacement should collapse to an observable caret");

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        textarea_id,
        Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: text_run_ids[2],
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: replaced_runs[0].0,
                    character_index: 0,
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
                    node: textarea_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: replaced_runs[0].0,
                    character_index: 0,
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
                    node: replaced_runs[0].0,
                    character_index: 99,
                },
                focus: accesskit::TextPosition {
                    node: replaced_runs[0].0,
                    character_index: 99,
                },
            },
        )),
    )));
    cx.run_until_parked();
    let invalid_targets = cx
        .latest_accessibility_tree_update()
        .expect("invalid selection targets should leave the tree intact");
    assert_eq!(
        a11y_node_by_id(&invalid_targets, textarea_id).value(),
        Some("aX\n")
    );
    assert_eq!(
        a11y_node_by_id(&invalid_targets, textarea_id).text_selection(),
        Some(&replaced_selection)
    );
    assert_eq!(changes.borrow().as_slice(), ["aX\n"]);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        textarea_id,
        Some(accesskit::ActionData::Value("x\r\ny\rz".into())),
    )));
    assert_eq!(changes.borrow().as_slice(), ["aX\n", "x\ny\nz"]);
    cx.run_until_parked();
    let normalized = cx
        .latest_accessibility_tree_update()
        .expect("normalized multiline value should publish");
    let normalized_textarea = a11y_node_by_id(&normalized, textarea_id);
    assert_eq!(normalized_textarea.value(), Some("x\ny\nz"));
    assert_eq!(
        a11y_text_run_children(&normalized, normalized_textarea)
            .iter()
            .map(|(_, node)| node.value().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["x\n", "y\n", "z"]
    );
}

#[open_gpui::test]
fn oversized_grapheme_degrades_to_whole_value_accessibility(cx: &mut open_gpui::TestAppContext) {
    struct OversizedGraphemeProbe {
        value: String,
    }

    impl Render for OversizedGraphemeProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(
                    TextInput::new("oversized-grapheme-input", "Oversized grapheme input")
                        .value(self.value.clone())
                        .on_change(|_, _, _| {}),
                )
                .child(
                    Textarea::new("oversized-grapheme-textarea", "Oversized grapheme textarea")
                        .value(self.value.clone())
                        .on_change(|_, _, _| {}),
                )
        }
    }

    let value = format!("a{}", "\u{301}".repeat(128));
    let (_, cx) = cx.add_window_view(|_, _| OversizedGraphemeProbe { value });
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("oversized grapheme input should publish");
    for label in ["Oversized grapheme input", "Oversized grapheme textarea"] {
        let (_, control) = a11y_node_with_label(&update, label);
        assert_exact_actions(
            control,
            &[accesskit::Action::Focus, accesskit::Action::SetValue],
        );
        assert!(control.text_selection().is_none());
        assert!(
            !control
                .children()
                .iter()
                .any(|id| { a11y_node_by_id(&update, *id).role() == accesskit::Role::TextRun })
        );
    }
}
