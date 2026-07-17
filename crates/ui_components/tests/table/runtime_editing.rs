use super::*;

#[open_gpui::test]
fn table_runtime_text_cell_edit_emits_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table_state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new("edit-runtime-table", "Edit runtime", table_state)
                .row_height(ui_px(32.0))
                .viewport_extent(ui_px(96.0))
                .on_cell_edit_change(move |change, _, _| {
                    edits.borrow_mut().push((
                        change
                            .source_row_id()
                            .expect("source-backed edit")
                            .as_str()
                            .to_owned(),
                        change.column_id().as_str().to_owned(),
                        change.source_index(),
                        change.previous_text().to_owned(),
                        change.next_text().to_owned(),
                    ));
                    let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                    *state_for_edit.borrow_mut() = next;
                })
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push(
                        activation
                            .source_row_id()
                            .expect("source-backed activation")
                            .as_str()
                            .to_owned(),
                    );
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(
                        selection
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
                    );
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    cx.update(init_text_input);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("status", "Ready")])
        .with_columns([
            TableColumn::new("name", "Name")
                .with_text_editable(true)
                .with_width(ui_px(180.0)),
            TableColumn::new("status", "Status").with_width(ui_px(120.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds(&table_source_cell_selector(
            "edit-runtime-table",
            "row-a",
            "status",
        ))
        .is_some(),
        "read-only cell should still render as a plain table cell"
    );
    assert!(
        cx.debug_bounds(&table_source_text_input_editor_selector(
            "edit-runtime-table",
            "row-a",
            "name",
        ))
        .is_some(),
        "editable name cell should render a nested text input with a stable selector"
    );
    assert!(
        cx.debug_bounds(&table_source_text_input_editor_selector(
            "edit-runtime-table",
            "row-a",
            "status",
        ))
        .is_none(),
        "read-only status cell must not mount a text input"
    );

    let input = cx
        .debug_bounds(&table_source_text_input_editor_selector(
            "edit-runtime-table",
            "row-a",
            "name",
        ))
        .expect("editable name input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("space");
    cx.simulate_input("Prime");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert!(
        edits.len() >= 2,
        "simulated text entry should emit controlled changes as the input value evolves"
    );
    assert!(
        edits.iter().all(|(row_id, column_id, source_index, _, _)| {
            row_id == "row-a" && column_id == "name" && *source_index == Some(0)
        }),
        "every edit payload should stay targeted by stable row and column ids"
    );
    assert_eq!(
        edits.first().cloned(),
        Some((
            "row-a".to_owned(),
            "name".to_owned(),
            Some(0),
            "Alpha".to_owned(),
            "Alpha ".to_owned(),
        ))
    );
    assert_eq!(
        edits.last().cloned(),
        Some((
            "row-a".to_owned(),
            "name".to_owned(),
            Some(0),
            "Alpha Prim".to_owned(),
            "Alpha Prime".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("name")))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Alpha Prime")
    );
    assert!(
        activations.borrow().is_empty(),
        "typing inside editable cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "typing inside editable cell must not toggle row selection"
    );
}

#[open_gpui::test]
fn table_runtime_multiline_cell_edit_emits_newline_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table_state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new(
                "multiline-edit-table",
                "Multiline edit runtime",
                table_state,
            )
            .row_height(ui_px(82.0))
            .viewport_extent(ui_px(120.0))
            .on_cell_edit_change(move |change, _, _| {
                edits.borrow_mut().push((
                    change
                        .source_row_id()
                        .expect("source-backed edit")
                        .as_str()
                        .to_owned(),
                    change.column_id().as_str().to_owned(),
                    change.source_index(),
                    change.previous_text().to_owned(),
                    change.next_text().to_owned(),
                ));
                let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                *state_for_edit.borrow_mut() = next;
            })
            .on_row_activate(move |activation, _, _| {
                activations.borrow_mut().push(
                    activation
                        .source_row_id()
                        .expect("source-backed activation")
                        .as_str()
                        .to_owned(),
                );
            })
            .on_row_selection_change(move |selection, _, _| {
                selections.borrow_mut().push(
                    selection
                        .source_row_id()
                        .expect("source-backed selection change")
                        .as_str()
                        .to_owned(),
                );
            });

            div().w(px(520.0)).h(px(180.0)).child(table)
        }
    }

    cx.update(init_text_input);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("notes", "Line 1")])
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(120.0)),
            TableColumn::new("notes", "Notes")
                .with_multiline_text_editor(3)
                .with_width(ui_px(280.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds(&table_source_textarea_editor_selector(
            "multiline-edit-table",
            "row-a",
            "notes",
        ))
        .is_some(),
        "multiline editable notes cell should render a nested textarea"
    );
    assert!(
        cx.debug_bounds(&table_source_text_input_editor_selector(
            "multiline-edit-table",
            "row-a",
            "notes",
        ))
        .is_none(),
        "multiline editable notes cell must not render the single-line text input"
    );

    let textarea = cx
        .debug_bounds(&table_source_textarea_editor_selector(
            "multiline-edit-table",
            "row-a",
            "notes",
        ))
        .expect("multiline notes textarea should expose a stable debug selector");
    cx.simulate_click(textarea.center(), Default::default());
    cx.simulate_keystrokes("enter");
    cx.simulate_input("Line 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert!(
        edits.len() >= 2,
        "simulated multiline entry should emit controlled changes as the textarea value evolves"
    );
    assert!(
        edits.iter().all(|(row_id, column_id, source_index, _, _)| {
            row_id == "row-a" && column_id == "notes" && *source_index == Some(0)
        }),
        "every multiline edit payload should stay targeted by stable row and column ids"
    );
    assert_eq!(
        edits.last().cloned(),
        Some((
            "row-a".to_owned(),
            "notes".to_owned(),
            Some(0),
            "Line 1\nLine ".to_owned(),
            "Line 1\nLine 2".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("notes")))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Line 1\nLine 2")
    );
    assert!(
        activations.borrow().is_empty(),
        "typing inside multiline editable cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "typing inside multiline editable cell must not toggle row selection"
    );
}

#[open_gpui::test]
fn table_runtime_boolean_cell_edit_emits_toggle_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new("bool-edit-runtime-table", "Bool edit runtime", state)
                .row_height(ui_px(32.0))
                .viewport_extent(ui_px(96.0))
                .on_cell_edit_change(move |change, _, _| {
                    edits.borrow_mut().push((
                        change
                            .source_row_id()
                            .expect("source-backed edit")
                            .as_str()
                            .to_owned(),
                        change.column_id().as_str().to_owned(),
                        change.source_index(),
                        change.previous_text().to_owned(),
                        change.next_text().to_owned(),
                    ));
                    let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                    *state_for_edit.borrow_mut() = next;
                })
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push(
                        activation
                            .source_row_id()
                            .expect("source-backed activation")
                            .as_str()
                            .to_owned(),
                    );
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(
                        selection
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
                    );
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("enabled", true)
            .with_cell("status", "Ready")])
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(180.0)),
            TableColumn::new("enabled", "Enabled")
                .with_checkbox_editor()
                .with_width(ui_px(96.0)),
            TableColumn::new("status", "Status").with_width(ui_px(120.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds(&table_source_checkbox_editor_selector(
            "bool-edit-runtime-table",
            "row-a",
            "enabled",
        ))
        .is_some(),
        "editable enabled cell should render a nested checkbox with a stable selector"
    );
    assert!(
        cx.debug_bounds(&table_source_text_input_editor_selector(
            "bool-edit-runtime-table",
            "row-a",
            "enabled",
        ))
        .is_none(),
        "boolean checkbox cell must not mount a text input"
    );
    assert!(
        cx.debug_bounds(&table_source_textarea_editor_selector(
            "bool-edit-runtime-table",
            "row-a",
            "enabled",
        ))
        .is_none(),
        "boolean checkbox cell must not mount a textarea"
    );

    let checkbox = cx
        .debug_bounds(&table_source_checkbox_editor_selector(
            "bool-edit-runtime-table",
            "row-a",
            "enabled",
        ))
        .expect("editable enabled checkbox should expose a stable debug selector");
    cx.simulate_click(checkbox.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert_eq!(
        edits.len(),
        1,
        "checkbox toggle should emit one controlled change"
    );
    assert_eq!(
        edits.first().cloned(),
        Some((
            "row-a".to_owned(),
            "enabled".to_owned(),
            Some(0),
            "true".to_owned(),
            "false".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("enabled"))),
        Some(&TableCellValue::Bool(false))
    );
    assert!(
        activations.borrow().is_empty(),
        "toggling a checkbox cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "toggling a checkbox cell must not toggle row selection"
    );
}

#[open_gpui::test]
fn table_runtime_select_cell_edit_emits_change_without_row_interaction(
    cx: &mut open_gpui::TestAppContext,
) {
    type EditLog = Vec<(String, String, Option<usize>, String, String)>;

    struct TestView {
        state: Rc<RefCell<TableState>>,
        edits: Rc<RefCell<EditLog>>,
        activations: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = self.state.borrow().clone();
            let state_for_edit = self.state.clone();
            let edits = self.edits.clone();
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let table = Table::new("select-edit-runtime-table", "Select edit runtime", state)
                .row_height(ui_px(32.0))
                .viewport_extent(ui_px(96.0))
                .on_cell_edit_change(move |change, _, _| {
                    edits.borrow_mut().push((
                        change
                            .source_row_id()
                            .expect("source-backed edit")
                            .as_str()
                            .to_owned(),
                        change.column_id().as_str().to_owned(),
                        change.source_index(),
                        change.previous_text().to_owned(),
                        change.next_text().to_owned(),
                    ));
                    let (next, outcome) = change.apply_to(state_for_edit.borrow().clone());
                    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
                    *state_for_edit.borrow_mut() = next;
                })
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push(
                        activation
                            .source_row_id()
                            .expect("source-backed activation")
                            .as_str()
                            .to_owned(),
                    );
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections.borrow_mut().push(
                        selection
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
                    );
                });

            div().w(px(460.0)).h(px(180.0)).child(table)
        }
    }

    cx.update(init_text_input);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("status", "ready")])
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(180.0)),
            TableColumn::new("status", "Status")
                .with_select_editor([
                    TableSelectOption::new("ready", "Ready"),
                    TableSelectOption::new("blocked", "Blocked"),
                ])
                .with_width(ui_px(120.0)),
        ])
        .with_pagination(TablePagination::disabled())
        .with_selection_activation_mode(TableSelectionActivationMode::RowClick),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        state: state.clone(),
        edits: edits.clone(),
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger_selector =
        table_source_select_editor_selector("select-edit-runtime-table", "row-a", "status");
    let content_selector = "select:Edit status for row row-a:select-content-scroll:content";
    let trigger = cx
        .debug_bounds(&trigger_selector)
        .expect("table select trigger should be rendered");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        activations.borrow().is_empty(),
        "clicking the select trigger should not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "clicking the select trigger should not toggle row selection"
    );

    if cx.debug_bounds(content_selector).is_none() {
        cx.simulate_keystrokes("space");
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    assert!(
        cx.debug_bounds(content_selector).is_some(),
        "select content should open from the table trigger"
    );

    let blocked_selector = required_table_source_select_option_selector(
        cx,
        "select-edit-runtime-table",
        "row-a",
        "status",
        "blocked",
    );
    let blocked = cx
        .debug_bounds(&blocked_selector)
        .expect("blocked option should be rendered in the table select popup");
    cx.simulate_click(blocked.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let edits = edits.borrow();
    assert_eq!(
        edits.len(),
        1,
        "select choice should emit one controlled change"
    );
    assert_eq!(
        edits.first().cloned(),
        Some((
            "row-a".to_owned(),
            "status".to_owned(),
            Some(0),
            "ready".to_owned(),
            "blocked".to_owned(),
        ))
    );
    assert_eq!(
        state
            .borrow()
            .rows()
            .first()
            .and_then(|row| row.cell(&TableColumnId::new("status")))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("blocked")
    );
    assert!(
        activations.borrow().is_empty(),
        "changing a select cell must not activate the row"
    );
    assert!(
        selections.borrow().is_empty(),
        "changing a select cell must not toggle row selection"
    );
}
