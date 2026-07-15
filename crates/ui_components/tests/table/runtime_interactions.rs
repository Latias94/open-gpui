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
                        change
                            .source_row_id()
                            .expect("source-backed selection change")
                            .as_str()
                            .to_owned(),
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
            TableSelectionScope::Row,
            Vec::<String>::new(),
        )],
        "row-click selection should emit the next selected-row ids without swallowing activation"
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
