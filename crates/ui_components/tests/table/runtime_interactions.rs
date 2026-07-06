use super::*;

#[open_gpui::test]
fn table_runtime_header_click_emits_sort_action(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new("sort-runtime-table", "Sort runtime", sample_table_state(12))
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_sort_requested(move |action, _, _| {
                    actions.borrow_mut().push(action);
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let score_header = cx
        .debug_bounds("table:sort-runtime-table:header:score")
        .expect("score header should render as an interactive sort target");
    cx.simulate_click(score_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].column_id().as_str(), "score");
    assert_eq!(actions[0].label(), "Score");
    assert_eq!(actions[0].current_direction(), None);
    assert_eq!(
        actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
    assert_eq!(actions[0].next_sorting()[0].column().as_str(), "score");
}

#[open_gpui::test]
fn table_runtime_row_click_and_tree_toggle_emit_controlled_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationLog = Vec<(String, String, usize, Option<bool>, bool)>;
    type ToggleLog = Vec<(String, bool, usize, Option<bool>)>;

    struct TestView {
        activations: Rc<RefCell<ActivationLog>>,
        toggles: Rc<RefCell<ToggleLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let toggles = self.toggles.clone();
            let state = TableState::new([TableRow::new("root")
                .with_cell("name", "Workspace")
                .with_cell("status", "Ready")
                .with_child(
                    TableRow::new("child")
                        .with_cell("name", "UI")
                        .with_cell("status", "Building"),
                )])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("tree-runtime-table", "Tree runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push((
                        activation.row_id().as_str().to_owned(),
                        activation.kind().as_str().to_owned(),
                        activation.action().depth(),
                        activation.action().tree_expanded(),
                        activation.action().modifiers().modified(),
                    ));
                })
                .on_row_expansion_request(move |toggle, _, _| {
                    toggles.borrow_mut().push((
                        toggle.row_id().as_str().to_owned(),
                        toggle.expanded(),
                        toggle.action().depth(),
                        toggle.action().tree_expanded(),
                    ));
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:tree-runtime-table:row:root")
        .expect("root row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        activations.borrow().as_slice(),
        &[("root".to_owned(), "click".to_owned(), 0, Some(false), false)]
    );
    assert!(toggles.borrow().is_empty());

    let toggle = cx
        .debug_bounds("table:tree-runtime-table:tree-toggle:root")
        .expect("root tree toggle should render");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().len(), 1);
    assert_eq!(
        toggles.borrow().as_slice(),
        &[("root".to_owned(), true, 0, Some(false))]
    );
}

#[open_gpui::test]
fn table_runtime_row_click_selection_is_controlled_and_preserves_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationLog = Vec<String>;
    type SelectionLog = Vec<(
        String,
        bool,
        TableSelectionMode,
        TableSelectionScope,
        Vec<String>,
    )>;

    struct TestView {
        activations: Rc<RefCell<ActivationLog>>,
        selections: Rc<RefCell<SelectionLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let selections = self.selections.clone();
            let state = TableState::new([
                TableRow::new("row-a")
                    .with_cell("name", "Alpha")
                    .with_cell("status", "Ready"),
                TableRow::new("row-b")
                    .with_cell("name", "Beta")
                    .with_cell("status", "Blocked"),
            ])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled())
            .with_selection_mode(TableSelectionMode::Multiple)
            .with_selection_activation_mode(TableSelectionActivationMode::RowClick)
            .with_selected_rows(["row-a"]);
            let table = Table::new("selection-runtime-table", "Selection runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections.borrow_mut().push((
                        change.row_id().as_str().to_owned(),
                        change.selected(),
                        change.selection_mode(),
                        change.scope(),
                        change
                            .current_selection()
                            .iter()
                            .map(|row_id| row_id.as_str().to_owned())
                            .collect(),
                    ));
                })
                .on_row_activate(move |activation, _, _| {
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:selection-runtime-table:row:row-a")
        .expect("selected row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().as_slice(), ["row-a"]);
    assert_eq!(
        selections.borrow().as_slice(),
        &[(
            "row-a".to_owned(),
            false,
            TableSelectionMode::Multiple,
            TableSelectionScope::Row,
            Vec::<String>::new(),
        )],
        "row-click selection should emit the next selected-row ids without swallowing activation"
    );
}

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
                        change.row_id().as_str().to_owned(),
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
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections
                        .borrow_mut()
                        .push(selection.row_id().as_str().to_owned());
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
        cx.debug_bounds("table:edit-runtime-table:cell:row-a:status")
            .is_some(),
        "read-only cell should still render as a plain table cell"
    );
    assert!(
        cx.debug_bounds("text-input:table:edit-runtime-table:cell:row-a:name:editor:root")
            .is_some(),
        "editable name cell should render a nested text input with a stable selector"
    );
    assert!(
        cx.debug_bounds("text-input:table:edit-runtime-table:cell:row-a:status:editor:root")
            .is_none(),
        "read-only status cell must not mount a text input"
    );

    let input = cx
        .debug_bounds("text-input:table:edit-runtime-table:cell:row-a:name:editor:root")
        .expect("editable name input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input(" Prime");
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
                    change.row_id().as_str().to_owned(),
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
                activations
                    .borrow_mut()
                    .push(activation.row_id().as_str().to_owned());
            })
            .on_row_selection_change(move |selection, _, _| {
                selections
                    .borrow_mut()
                    .push(selection.row_id().as_str().to_owned());
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
        cx.debug_bounds("textarea:table:multiline-edit-table:cell:row-a:notes:editor:root")
            .is_some(),
        "multiline editable notes cell should render a nested textarea"
    );
    assert!(
        cx.debug_bounds("text-input:table:multiline-edit-table:cell:row-a:notes:editor:root")
            .is_none(),
        "multiline editable notes cell must not render the single-line text input"
    );

    let textarea = cx
        .debug_bounds("textarea:table:multiline-edit-table:cell:row-a:notes:editor:root")
        .expect("multiline notes textarea should expose a stable debug selector");
    cx.simulate_click(textarea.center(), Default::default());
    cx.simulate_input("\nLine 2");
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
                        change.row_id().as_str().to_owned(),
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
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections
                        .borrow_mut()
                        .push(selection.row_id().as_str().to_owned());
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
        cx.debug_bounds("checkbox:table:bool-edit-runtime-table:cell:row-a:enabled:editor:root")
            .is_some(),
        "editable enabled cell should render a nested checkbox with a stable selector"
    );
    assert!(
        cx.debug_bounds("text-input:table:bool-edit-runtime-table:cell:row-a:enabled:editor:root")
            .is_none(),
        "boolean checkbox cell must not mount a text input"
    );
    assert!(
        cx.debug_bounds("textarea:table:bool-edit-runtime-table:cell:row-a:enabled:editor:root")
            .is_none(),
        "boolean checkbox cell must not mount a textarea"
    );

    let checkbox = cx
        .debug_bounds("checkbox:table:bool-edit-runtime-table:cell:row-a:enabled:editor:root")
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
                        change.row_id().as_str().to_owned(),
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
                    activations
                        .borrow_mut()
                        .push(activation.row_id().as_str().to_owned());
                })
                .on_row_selection_change(move |selection, _, _| {
                    selections
                        .borrow_mut()
                        .push(selection.row_id().as_str().to_owned());
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
        "select:table:select-edit-runtime-table:cell:row-a:status:editor:trigger";
    let content_selector = "select:Edit status for row row-a:select-content-scroll:content";
    let trigger = cx
        .debug_bounds(trigger_selector)
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

    let blocked = cx
        .debug_bounds("listbox:table:select-edit-runtime-table:cell:row-a:status:editor-listbox:option:blocked")
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

#[open_gpui::test]
fn table_runtime_explicit_control_selection_ignores_row_click(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let state = sample_table_state(4)
                .with_selection_activation_mode(TableSelectionActivationMode::ExplicitControl)
                .with_selected_rows(["row-0001"]);
            let table = Table::new("explicit-selection-table", "Explicit selection", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections
                        .borrow_mut()
                        .push(change.row_id().as_str().to_owned());
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row = cx
        .debug_bounds("table:explicit-selection-table:row:row-0001")
        .expect("selected row should render");
    cx.simulate_click(row.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        selections.borrow().is_empty(),
        "explicit-control selection should wait for checkbox/radio chrome instead of row clicks"
    );
}

#[open_gpui::test]
fn table_runtime_unloaded_branch_toggle_emits_child_load_metadata(
    cx: &mut open_gpui::TestAppContext,
) {
    type ToggleLog = Vec<(String, bool, usize, Option<String>, bool, usize)>;

    struct TestView {
        toggles: Rc<RefCell<ToggleLog>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let toggles = self.toggles.clone();
            let state = TableState::new([TableRow::new("remote")
                .with_cell("name", "Remote workspace")
                .with_cell("status", "Retry")
                .with_children_load_failed("Network unavailable")])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(180.0)),
                TableColumn::new("status", "Status").with_width(ui_px(120.0)),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("remote-runtime-table", "Remote runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_expansion_request(move |toggle, _, _| {
                    let load_state = toggle
                        .children_load_state()
                        .and_then(TableRowChildrenLoadState::message)
                        .map(str::to_owned);
                    let failed = toggle
                        .children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_failed);
                    toggles.borrow_mut().push((
                        toggle.row_id().as_str().to_owned(),
                        toggle.expanded(),
                        toggle.action().depth(),
                        load_state,
                        failed,
                        toggle.loaded_child_count(),
                    ));
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let toggle = cx
        .debug_bounds("table:remote-runtime-table:tree-toggle:remote")
        .expect("remote branch tree toggle should render without loaded children");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        &[(
            "remote".to_owned(),
            true,
            0,
            Some("Network unavailable".to_owned()),
            true,
            0,
        )]
    );
}
