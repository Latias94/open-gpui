use super::*;
use open_gpui::accesskit;

fn node_with_role_and_label<'a>(
    update: &'a accesskit::TreeUpdate,
    role: accesskit::Role,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} node labelled {label:?}"))
}

fn node_with_role_and_row_index(
    update: &accesskit::TreeUpdate,
    role: accesskit::Role,
    row_index: usize,
) -> (accesskit::NodeId, &accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.row_index() == Some(row_index))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} node at row {row_index}"))
}

#[open_gpui::test]
fn table_toolbar_runtime_final_accessibility_tree(cx: &mut open_gpui::TestAppContext) {
    struct TableToolbarA11yProbe {
        summary: Option<&'static str>,
    }

    impl Render for TableToolbarA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let toolbar = TableToolbar::new("semantic-table-toolbar", "Table filters");
            let toolbar = if let Some(summary) = self.summary {
                toolbar.summary(summary)
            } else {
                toolbar
            };
            div().child(toolbar)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TableToolbarA11yProbe {
        summary: Some("2 rows visible"),
    });

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("table toolbar accessibility tree should publish");
    let (toolbar_id, toolbar) =
        node_with_role_and_label(&update, accesskit::Role::Toolbar, "Table filters");
    assert_eq!(toolbar.description(), Some("2 rows visible"));
    assert!(!toolbar.supports_action(accesskit::Action::Click));
    assert!(!toolbar.supports_action(accesskit::Action::Focus));

    view.update(cx, |probe, cx| {
        probe.summary = Some("3 rows visible");
        cx.notify();
    });
    cx.run_until_parked();
    let updated = cx
        .latest_accessibility_tree_update()
        .expect("updated table toolbar accessibility tree should publish");
    let (updated_id, updated_toolbar) =
        node_with_role_and_label(&updated, accesskit::Role::Toolbar, "Table filters");
    assert_eq!(updated_id, toolbar_id);
    assert_eq!(updated_toolbar.description(), Some("3 rows visible"));

    view.update(cx, |probe, cx| {
        probe.summary = None;
        cx.notify();
    });
    cx.run_until_parked();
    let cleared = cx
        .latest_accessibility_tree_update()
        .expect("cleared table toolbar accessibility tree should publish");
    let (cleared_id, cleared_toolbar) =
        node_with_role_and_label(&cleared, accesskit::Role::Toolbar, "Table filters");
    assert_eq!(cleared_id, toolbar_id);
    assert_eq!(cleared_toolbar.description(), None);
}

#[open_gpui::test]
fn table_runtime_final_accessibility_tree(cx: &mut open_gpui::TestAppContext) {
    struct TableA11yProbe {
        sort_actions: Rc<RefCell<Vec<TableHeaderAction>>>,
        row_activations: Rc<RefCell<Vec<(String, String, usize, Option<bool>)>>>,
        expansion_toggles: Rc<RefCell<Vec<(String, bool, usize, Option<bool>)>>>,
        sort_enabled: bool,
        name: String,
    }

    impl Render for TableA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let sort_actions = self.sort_actions.clone();
            let row_activations = self.row_activations.clone();
            let expansion_toggles = self.expansion_toggles.clone();
            let state = TableState::new([
                TableRow::new("alpha")
                    .with_cell("name", self.name.clone())
                    .with_cell("score", 10_usize)
                    .with_child(
                        TableRow::new("alpha-child")
                            .with_cell("name", "Alpha child")
                            .with_cell("score", 5_usize),
                    ),
                TableRow::new("beta")
                    .with_cell("name", "Beta")
                    .with_cell("score", 20_usize),
            ])
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("score", "Score"),
            ])
            .with_selected_rows(["alpha"]);
            let table = Table::new("semantic-table", "Release table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_activate(move |activation, _, _| {
                    row_activations.borrow_mut().push((
                        activation.row_id().as_str().to_owned(),
                        activation.kind().as_str().to_owned(),
                        activation.action().depth(),
                        activation.action().tree_expanded(),
                    ));
                })
                .on_row_expansion_request(move |toggle, _, _| {
                    expansion_toggles.borrow_mut().push((
                        toggle.row_id().as_str().to_owned(),
                        toggle.expanded(),
                        toggle.action().depth(),
                        toggle.action().tree_expanded(),
                    ));
                });
            let table = if self.sort_enabled {
                table.on_sort_requested(move |action, _, _| {
                    sort_actions.borrow_mut().push(action);
                })
            } else {
                table
            };

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let sort_actions = Rc::new(RefCell::new(Vec::new()));
    let row_activations = Rc::new(RefCell::new(Vec::new()));
    let expansion_toggles = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TableA11yProbe {
        sort_actions: sort_actions.clone(),
        row_activations: row_activations.clone(),
        expansion_toggles: expansion_toggles.clone(),
        sort_enabled: true,
        name: "Alpha".to_owned(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("table accessibility tree should publish");
    let (table_id, table_node) =
        node_with_role_and_label(&initial, accesskit::Role::Table, "Release table");
    assert_eq!(table_node.row_count(), Some(3));
    assert_eq!(table_node.column_count(), Some(2));
    assert!(!table_node.supports_action(accesskit::Action::Click));
    assert!(!table_node.supports_action(accesskit::Action::Focus));

    let (header_row_id, header_row) =
        node_with_role_and_row_index(&initial, accesskit::Role::Row, 1);
    assert!(!header_row.supports_action(accesskit::Action::Click));
    assert!(!header_row.supports_action(accesskit::Action::Focus));
    let (header_id, header) =
        node_with_role_and_label(&initial, accesskit::Role::ColumnHeader, "Name");
    assert_eq!(header.column_index(), Some(1));
    assert_eq!(header.column_span(), Some(1));
    assert_eq!(header.row_span(), Some(1));
    assert_eq!(header.sort_direction(), None);
    assert!(header.supports_action(accesskit::Action::Click));
    assert!(header.supports_action(accesskit::Action::Focus));

    let (row_id, row) = node_with_role_and_row_index(&initial, accesskit::Role::Row, 2);
    assert_eq!(row.is_selected(), Some(true));
    assert!(row.supports_action(accesskit::Action::Click));
    assert!(row.supports_action(accesskit::Action::Focus));
    let row_cells = row
        .children()
        .iter()
        .filter_map(|child_id| initial.nodes.iter().find(|(id, _)| id == child_id))
        .filter(|(_, node)| node.role() == accesskit::Role::Cell)
        .collect::<Vec<_>>();
    assert_eq!(row_cells.len(), 2);
    assert_eq!(row_cells[0].1.column_index(), Some(1));
    assert_eq!(row_cells[1].1.column_index(), Some(2));
    assert_eq!(row_cells[0].1.value(), Some("Alpha"));
    assert_eq!(row_cells[1].1.value(), Some("10"));
    let name_cell_id = row_cells[0].0;
    assert!(
        row_cells
            .iter()
            .all(|(_, cell)| !cell.supports_action(accesskit::Action::Click)
                && !cell.supports_action(accesskit::Action::Focus))
    );

    let (toggle_id, toggle) =
        node_with_role_and_label(&initial, accesskit::Role::Button, "Expand row alpha");
    assert_eq!(toggle.is_expanded(), Some(false));
    assert!(toggle.supports_action(accesskit::Action::Click));
    assert!(!toggle.supports_action(accesskit::Action::Focus));
    assert!(table_node.children().contains(&header_row_id));
    assert!(table_node.children().contains(&row_id));
    assert!(header_row.children().contains(&header_id));
    assert!(row_cells[0].1.children().contains(&toggle_id));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: header_id,
        data: None,
    }));
    let recorded_sort_actions = sort_actions.borrow();
    assert_eq!(recorded_sort_actions.len(), 1);
    assert_eq!(recorded_sort_actions[0].column_id().as_str(), "name");
    assert_eq!(recorded_sort_actions[0].current_direction(), None);
    assert_eq!(
        recorded_sort_actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
    assert_eq!(recorded_sort_actions[0].next_sorting().len(), 1);
    assert_eq!(
        recorded_sort_actions[0].next_sorting()[0].column().as_str(),
        "name"
    );
    assert_eq!(
        recorded_sort_actions[0].next_sorting()[0].direction(),
        TableSortDirection::Ascending
    );
    drop(recorded_sort_actions);

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: row_id,
        data: None,
    }));
    assert_eq!(
        row_activations.borrow().as_slice(),
        &[("alpha".to_owned(), "click".to_owned(), 0, Some(false))]
    );

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: header_id,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("header focus should publish")
            .focus,
        header_id
    );

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: row_id,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("row focus should publish")
            .focus,
        row_id
    );

    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let rerendered = cx
        .latest_accessibility_tree_update()
        .expect("rerendered table accessibility tree should publish");
    assert_eq!(
        node_with_role_and_label(&rerendered, accesskit::Role::Table, "Release table").0,
        table_id
    );
    assert_eq!(
        node_with_role_and_row_index(&rerendered, accesskit::Role::Row, 1).0,
        header_row_id
    );
    assert_eq!(
        node_with_role_and_label(&rerendered, accesskit::Role::ColumnHeader, "Name").0,
        header_id
    );
    assert_eq!(
        node_with_role_and_row_index(&rerendered, accesskit::Role::Row, 2).0,
        row_id
    );

    view.update(cx, |probe, cx| {
        probe.name = "Alpha updated".to_owned();
        cx.notify();
    });
    cx.run_until_parked();
    let value_updated = cx
        .latest_accessibility_tree_update()
        .expect("updated table cell value should publish");
    let (_, updated_row) = node_with_role_and_row_index(&value_updated, accesskit::Role::Row, 2);
    let (updated_name_cell_id, updated_name_cell) = updated_row
        .children()
        .iter()
        .filter_map(|child_id| value_updated.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(1))
        .map(|(id, node)| (*id, node))
        .expect("updated name cell should publish");
    assert_eq!(updated_name_cell_id, name_cell_id);
    assert_eq!(updated_name_cell.value(), Some("Alpha updated"));

    view.update(cx, |probe, cx| {
        probe.sort_enabled = false;
        cx.notify();
    });
    cx.run_until_parked();
    let non_sortable = cx
        .latest_accessibility_tree_update()
        .expect("non-sortable table accessibility tree should publish");
    let (non_sortable_header_id, non_sortable_header) =
        node_with_role_and_label(&non_sortable, accesskit::Role::ColumnHeader, "Name");
    assert_eq!(non_sortable_header_id, header_id);
    assert!(!non_sortable_header.supports_action(accesskit::Action::Click));
    assert!(!non_sortable_header.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: header_id,
        data: None,
    }));
    assert_eq!(sort_actions.borrow().len(), 1);

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: toggle_id,
        data: None,
    }));
    assert_eq!(
        expansion_toggles.borrow().as_slice(),
        &[("alpha".to_owned(), true, 0, Some(false))]
    );
}

#[open_gpui::test]
fn nested_table_headers_preserve_rows_spans_sort_and_segment_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    struct NestedHeaderA11yProbe {
        descending: bool,
    }

    impl Render for NestedHeaderA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let sorting = if self.descending {
                TableSort::descending("score")
            } else {
                TableSort::ascending("score")
            };
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "UI")
                .with_cell("score", 42_usize)
                .with_cell("status", "Ready")])
            .with_column_tree([
                TableColumnGroup::new(
                    "identity",
                    "Identity",
                    [
                        TableColumn::new("name", "Name"),
                        TableColumn::new("team", "Team"),
                    ],
                ),
                TableColumnGroup::new(
                    "metrics",
                    "Metrics",
                    [TableColumnGroup::new(
                        "scores",
                        "Scores",
                        [
                            TableColumn::new("score", "Score").with_sortable(true),
                            TableColumn::new("status", "Status"),
                        ],
                    )],
                ),
            ])
            .with_column_order(["name", "score", "team", "status"])
            .with_sorting([sorting]);

            div().w(px(640.0)).h(px(260.0)).child(
                Table::new("nested-semantic-table", "Nested release table", state)
                    .row_height(ui_px(24.0))
                    .viewport_extent(ui_px(96.0))
                    .on_sort_requested(|_, _, _| {}),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| NestedHeaderA11yProbe { descending: false });

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("nested table accessibility tree should publish");
    let (table_id, table) =
        node_with_role_and_label(&update, accesskit::Role::Table, "Nested release table");
    assert_eq!(table.row_count(), Some(4));
    assert_eq!(table.column_count(), Some(4));

    let header_rows = (1..=3)
        .map(|row_index| node_with_role_and_row_index(&update, accesskit::Role::Row, row_index))
        .collect::<Vec<_>>();
    let (body_row_id, _) = node_with_role_and_row_index(&update, accesskit::Role::Row, 4);
    assert!(
        header_rows
            .iter()
            .all(|(row_id, _)| table.children().contains(row_id))
    );
    assert!(table.children().contains(&body_row_id));
    assert_ne!(table_id, body_row_id);

    let identity_segments = update
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.role() == accesskit::Role::ColumnHeader && node.label() == Some("Identity")
        })
        .collect::<Vec<_>>();
    assert_eq!(identity_segments.len(), 2);
    assert_ne!(identity_segments[0].0, identity_segments[1].0);
    assert!(
        identity_segments
            .iter()
            .all(|(_, node)| node.column_span() == Some(1))
    );

    let (name_id, name) = node_with_role_and_label(&update, accesskit::Role::ColumnHeader, "Name");
    assert_eq!(name.row_span(), Some(2));
    assert_eq!(name.column_span(), Some(1));

    let (score_id, score) =
        node_with_role_and_label(&update, accesskit::Role::ColumnHeader, "Score");
    assert_eq!(
        score.sort_direction(),
        Some(accesskit::SortDirection::Ascending)
    );
    assert_eq!(score.row_span(), Some(1));
    assert_eq!(score.column_span(), Some(1));
    assert!(score.supports_action(accesskit::Action::Click));
    assert!(score.supports_action(accesskit::Action::Focus));

    assert!(
        header_rows
            .iter()
            .any(|(_, row)| row.children().contains(&name_id))
    );
    assert!(
        header_rows
            .iter()
            .any(|(_, row)| row.children().contains(&score_id))
    );

    view.update(cx, |probe, cx| {
        probe.descending = true;
        cx.notify();
    });
    cx.run_until_parked();
    let descending = cx
        .latest_accessibility_tree_update()
        .expect("descending nested table tree should publish");
    let (descending_score_id, descending_score) =
        node_with_role_and_label(&descending, accesskit::Role::ColumnHeader, "Score");
    assert_eq!(descending_score_id, score_id);
    assert_eq!(
        descending_score.sort_direction(),
        Some(accesskit::SortDirection::Descending)
    );
}

#[open_gpui::test]
fn table_header_rows_keep_identity_when_column_pinning_changes(cx: &mut open_gpui::TestAppContext) {
    struct HeaderPinningA11yProbe {
        pinned: bool,
    }

    impl Render for HeaderPinningA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "UI")
                .with_cell("score", 42_usize)
                .with_cell("status", "Ready")])
            .with_column_tree([
                TableColumnGroup::new(
                    "identity",
                    "Identity",
                    [
                        TableColumn::new("name", "Name"),
                        TableColumn::new("team", "Team"),
                    ],
                ),
                TableColumnGroup::new(
                    "delivery",
                    "Delivery",
                    [
                        TableColumn::new("score", "Score"),
                        TableColumn::new("status", "Status"),
                    ],
                ),
            ]);
            let state = if self.pinned {
                state.with_column_pinning(
                    TableColumnPinning::new()
                        .pinned_left(["name"])
                        .pinned_right(["status"]),
                )
            } else {
                state
            };

            div().w(px(640.0)).h(px(220.0)).child(
                Table::new("pinning-semantic-table", "Pinning semantic table", state)
                    .row_height(ui_px(24.0))
                    .viewport_extent(ui_px(96.0)),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| HeaderPinningA11yProbe { pinned: false });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial header rows should publish");
    let initial_header_rows = (1..=2)
        .map(|row_index| node_with_role_and_row_index(&initial, accesskit::Role::Row, row_index).0)
        .collect::<Vec<_>>();

    view.update(cx, |probe, cx| {
        probe.pinned = true;
        cx.notify();
    });
    cx.run_until_parked();
    let pinned = cx
        .latest_accessibility_tree_update()
        .expect("pinned header rows should publish");
    let pinned_header_rows = (1..=2)
        .map(|row_index| node_with_role_and_row_index(&pinned, accesskit::Role::Row, row_index).0)
        .collect::<Vec<_>>();

    assert_eq!(pinned_header_rows, initial_header_rows);
}

#[open_gpui::test]
fn table_virtual_windows_recycle_without_reusing_semantic_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    struct VirtualTableA11yProbe;

    impl Render for VirtualTableA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "semantic-virtual-table",
                "Virtual release table",
                sample_center_window_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| VirtualTableA11yProbe);
    assert!(cx.activate_accessibility());
    cx.update(|window, cx| window.draw(cx).clear());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial virtual table tree should publish");
    let (table_id, table) =
        node_with_role_and_label(&initial, accesskit::Role::Table, "Virtual release table");
    assert_eq!(table.row_count(), Some(81));
    assert_eq!(table.column_count(), Some(8));
    let (row_zero_id, row_zero) = node_with_role_and_row_index(&initial, accesskit::Role::Row, 2);
    let (metric_zero_id, metric_zero) = row_zero
        .children()
        .iter()
        .filter_map(|child_id| initial.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(2))
        .map(|(id, node)| (*id, node))
        .expect("initial center cell should publish");
    assert_eq!(metric_zero.column_index(), Some(2));
    assert_eq!(metric_zero.value(), Some("10"));

    let horizontal_viewport = cx
        .debug_bounds("scroll-area:table:semantic-virtual-table:row-center-scroll:row-0000")
        .expect("body center lane should expose a horizontal viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: horizontal_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let horizontal = cx
        .latest_accessibility_tree_update()
        .expect("horizontal virtual window should publish");
    let (horizontal_table_id, horizontal_table) =
        node_with_role_and_label(&horizontal, accesskit::Role::Table, "Virtual release table");
    assert_eq!(horizontal_table_id, table_id);
    assert_eq!(horizontal_table.row_count(), Some(81));
    assert_eq!(horizontal_table.column_count(), Some(8));
    let (horizontal_row_id, horizontal_row) =
        node_with_role_and_row_index(&horizontal, accesskit::Role::Row, 2);
    assert_eq!(horizontal_row_id, row_zero_id);
    assert!(!horizontal.nodes.iter().any(|(id, _)| *id == metric_zero_id));
    let (metric_five_id, metric_five) = horizontal_row
        .children()
        .iter()
        .filter_map(|child_id| horizontal.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(7))
        .map(|(id, node)| (*id, node))
        .expect("far center cell should replace the initial center cell");
    assert_ne!(metric_five_id, metric_zero_id);
    assert_eq!(metric_five.column_index(), Some(7));
    assert_eq!(metric_five.value(), Some("60"));

    let pinned_cell = cx
        .debug_bounds("table:semantic-virtual-table:cell:row-0000:name")
        .expect("left pinned cell should remain mounted");
    cx.simulate_event(ScrollWheelEvent {
        position: pinned_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let vertical = cx
        .latest_accessibility_tree_update()
        .expect("vertical virtual window should publish");
    let (vertical_table_id, vertical_table) =
        node_with_role_and_label(&vertical, accesskit::Role::Table, "Virtual release table");
    assert_eq!(vertical_table_id, table_id);
    assert_eq!(vertical_table.row_count(), Some(81));
    assert_eq!(vertical_table.column_count(), Some(8));
    assert!(
        !vertical.nodes.iter().any(|(id, _)| {
            *id == row_zero_id || *id == metric_five_id || *id == metric_zero_id
        })
    );
    let (row_ten_id, row_ten) = node_with_role_and_row_index(&vertical, accesskit::Role::Row, 12);
    assert_ne!(row_ten_id, row_zero_id);
    assert!(vertical_table.children().contains(&row_ten_id));
    let metric_ten = row_ten
        .children()
        .iter()
        .filter_map(|child_id| vertical.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(7))
        .map(|(_, node)| node)
        .expect("vertically recycled metric cell should publish");
    assert_eq!(metric_ten.value(), Some("70"));
}
