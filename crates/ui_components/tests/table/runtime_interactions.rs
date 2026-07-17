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
                        activation
                            .source_row_id()
                            .expect("source-backed activation")
                            .as_str()
                            .to_owned(),
                        activation.kind().as_str().to_owned(),
                        activation.action().depth(),
                        activation.action().tree_expanded(),
                        activation.action().modifiers().modified(),
                    ));
                })
                .on_row_expansion_request(move |toggle, _, _| {
                    toggles.borrow_mut().push((
                        toggle
                            .source_row_id()
                            .expect("source-backed expansion toggle")
                            .as_str()
                            .to_owned(),
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
        .debug_bounds(&table_source_row_selector("tree-runtime-table", "root"))
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
        .debug_bounds(&table_source_tree_toggle_selector(
            "tree-runtime-table",
            "root",
        ))
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
    assert!(
        cx.debug_bounds(&table_source_row_selector("tree-runtime-table", "child"))
            .is_none(),
        "a controlled expansion request must not mutate hidden table state"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        &[
            ("root".to_owned(), true, 0, Some(false)),
            ("root".to_owned(), true, 0, Some(false)),
        ],
        "a refused controlled request must leave keyboard intent resolved from caller state"
    );
    assert!(
        cx.debug_bounds(&table_source_row_selector("tree-runtime-table", "child"))
            .is_none(),
        "keyboard expansion intent must also wait for a caller state commit"
    );
}

#[open_gpui::test]
fn table_runtime_row_click_selection_is_controlled_and_preserves_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    type ActivationLog = Vec<String>;
    type SelectionLog = Vec<(String, bool, TableSelectionMode, Vec<String>)>;

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
            .with_selected_rows([table_source_selection_identity("row-a")]);
            let table = Table::new("selection-runtime-table", "Selection runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections.borrow_mut().push((
                        change
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
                        change.selected(),
                        change.selection_mode(),
                        change
                            .current_selection()
                            .iter()
                            .map(|identity| identity.row_id().as_str().to_owned())
                            .collect(),
                    ));
                })
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push(
                        activation
                            .source_row_id()
                            .expect("source-backed activation")
                            .as_str()
                            .to_owned(),
                    );
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
        .debug_bounds(&table_source_row_selector(
            "selection-runtime-table",
            "row-a",
        ))
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
            Vec::<String>::new(),
        )],
        "row-click selection should emit the next selected-row ids without swallowing activation"
    );
}

#[open_gpui::test]
fn table_runtime_duplicate_row_selection_emits_exact_controlled_identities(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<Vec<TableSourceRowIdentity>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let state = TableState::new([
                TableRow::new("duplicate")
                    .with_instance_id("first")
                    .with_cell("name", "First"),
                TableRow::new("duplicate")
                    .with_instance_id("second")
                    .with_cell("name", "Second"),
            ])
            .with_columns([TableColumn::new("name", "Name").with_width(ui_px(180.0))])
            .with_pagination(TablePagination::disabled())
            .with_selection_mode(TableSelectionMode::Multiple)
            .with_selection_activation_mode(TableSelectionActivationMode::RowClick)
            .with_selected_rows([TableSourceRowIdentity::explicit("duplicate", "second")]);
            let table = Table::new(
                "duplicate-selection-runtime-table",
                "Duplicate selection runtime",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .on_row_selection_change(move |change, _, _| {
                selections
                    .borrow_mut()
                    .push(change.current_selection().to_vec());
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

    let first_identity = TableRowIdentity::source_instance("duplicate", "first");
    let first = cx
        .debug_bounds(&TableDebugSelector::row(
            "duplicate-selection-runtime-table",
            &first_identity,
        ))
        .expect("the first duplicate row should render with its exact selector");
    for _ in 0..2 {
        cx.simulate_click(first.center(), Default::default());
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    let expected = vec![
        TableSourceRowIdentity::explicit("duplicate", "first"),
        TableSourceRowIdentity::explicit("duplicate", "second"),
    ];
    assert_eq!(
        selections.borrow().as_slice(),
        &[expected.clone(), expected],
        "a refused controlled commit must not merge duplicate ids or change hidden selection state"
    );
}

#[open_gpui::test]
fn table_runtime_descendant_selection_emits_explicit_roots_and_commits_cleanly(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected_rows: Vec<TableSourceRowIdentity>,
        changes: Rc<RefCell<Vec<TableRowSelectionChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = TableState::new([TableRow::new("parent")
                .with_cell("name", "Parent")
                .with_child(TableRow::new("child").with_cell("name", "Child"))
                .with_child(TableRow::new("sibling").with_cell("name", "Sibling"))])
            .with_columns([TableColumn::new("name", "Name").with_width(ui_px(180.0))])
            .with_pagination(TablePagination::disabled())
            .with_all_rows_expanded()
            .with_selection_policy(TableSelectionPolicy::new(
                TableSelectionMode::Multiple,
                TableSelectionActivationMode::RowClick,
                TableSubRowSelectionPolicy::Descendants,
            ))
            .with_selected_rows(self.selected_rows.clone());
            let table = Table::new(
                "descendant-selection-runtime-table",
                "Descendant selection runtime",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .on_row_selection_change(move |change, _, _| {
                changes.borrow_mut().push(change);
            });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        selected_rows: vec![table_source_selection_identity("parent")],
        changes: changes.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let parent = cx
        .debug_bounds(&table_source_row_selector(
            "descendant-selection-runtime-table",
            "parent",
        ))
        .expect("selected parent row should render");
    let child = cx
        .debug_bounds(&table_source_row_selector(
            "descendant-selection-runtime-table",
            "child",
        ))
        .expect("inherited selected child row should render");

    cx.simulate_click(parent.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_click(child.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    let changes_snapshot = changes.borrow().clone();
    assert_eq!(changes_snapshot.len(), 2);
    assert!(!changes_snapshot[0].selected());
    assert!(changes_snapshot[0].current_selection().is_empty());
    assert!(!changes_snapshot[1].selected());
    assert!(
        changes_snapshot[1].current_selection().is_empty(),
        "canceling an inherited child selection must remove its explicit selected ancestor"
    );

    view.update(cx, |view, cx| {
        view.selected_rows = changes_snapshot[1].current_selection().to_vec();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let child = cx
        .debug_bounds(&table_source_row_selector(
            "descendant-selection-runtime-table",
            "child",
        ))
        .expect("child row should remain rendered after committing deselection");
    cx.simulate_click(child.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    let changes = changes.borrow();
    let committed_child_change = changes.last().expect("child re-selection should emit");
    assert!(committed_child_change.selected());
    assert_eq!(
        committed_child_change.current_selection(),
        [table_source_selection_identity("child")]
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
                .with_selected_rows([table_source_selection_identity("row-0001")]);
            let table = Table::new("explicit-selection-table", "Explicit selection", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_selection_change(move |change, _, _| {
                    selections.borrow_mut().push(
                        change
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
                    );
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
        .debug_bounds(&table_source_row_selector(
            "explicit-selection-table",
            "row-0001",
        ))
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
                        toggle
                            .source_row_id()
                            .expect("source-backed expansion toggle")
                            .as_str()
                            .to_owned(),
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
        .debug_bounds(&table_source_tree_toggle_selector(
            "remote-runtime-table",
            "remote",
        ))
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
