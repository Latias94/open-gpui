use super::*;

#[open_gpui::test]
fn table_logical_nodes_keep_identity_when_row_and_column_pinning_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct PinningA11yProbe {
        column_pinned: bool,
        row_pinned: bool,
        sort_actions: Rc<RefCell<Vec<TableHeaderAction>>>,
        row_activations: Rc<RefCell<Vec<TableRowIdentity>>>,
    }

    impl Render for PinningA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let sort_actions = self.sort_actions.clone();
            let row_activations = self.row_activations.clone();
            let mut state = TableState::new([
                TableRow::new("alpha")
                    .with_cell("name", "Alpha")
                    .with_cell("score", 10_usize)
                    .with_cell("status", "Ready"),
                TableRow::new("beta")
                    .with_cell("name", "Beta")
                    .with_cell("score", 20_usize)
                    .with_cell("status", "Queued"),
            ])
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("score", "Score").with_sortable(true),
                TableColumn::new("status", "Status"),
            ])
            .with_pagination(TablePagination::disabled());
            if self.column_pinned {
                state = state.with_column_pinning(
                    TableColumnPinning::new()
                        .pinned_left(["name"])
                        .pinned_right(["status"]),
                );
            }
            if self.row_pinned {
                state = state.with_row_pinning(
                    TableRowPinning::new().pinned_top([table_source_row_identity("beta")]),
                );
            }

            div().w(px(640.0)).h(px(220.0)).child(
                Table::new("pinning-semantic-table", "Pinning semantic table", state)
                    .row_height(ui_px(24.0))
                    .viewport_extent(ui_px(96.0))
                    .on_sort_requested(move |action, _, _| {
                        sort_actions.borrow_mut().push(action);
                    })
                    .on_row_activate(move |activation, _, _| {
                        row_activations
                            .borrow_mut()
                            .push(activation.identity().clone());
                    }),
            )
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct LogicalNodeIds {
        table: accesskit::NodeId,
        header_row: accesskit::NodeId,
        name_header: accesskit::NodeId,
        score_header: accesskit::NodeId,
        beta_row: accesskit::NodeId,
        beta_name_cell: accesskit::NodeId,
        beta_score_cell: accesskit::NodeId,
    }

    fn logical_node_ids(update: &accesskit::TreeUpdate) -> LogicalNodeIds {
        let (table, table_node) =
            node_with_role_and_label(update, accesskit::Role::Table, "Pinning semantic table");
        let (header_row, header_row_node) =
            node_with_role_and_row_index(update, accesskit::Role::Row, 1);
        let name_header = node_with_role_and_label(update, accesskit::Role::ColumnHeader, "Name").0;
        let score_header =
            node_with_role_and_label(update, accesskit::Role::ColumnHeader, "Score").0;
        let beta_name_cell = cell_with_column_and_value(update, 1, "Beta").0;
        let beta_score_cell = cell_with_column_and_value(update, 2, "20").0;
        let (beta_row, beta_row_node) =
            parent_with_role(update, beta_name_cell, accesskit::Role::Row);
        assert_eq!(
            parent_with_role(update, beta_score_cell, accesskit::Role::Row).0,
            beta_row,
            "both Beta cells must remain under the same semantic row"
        );
        assert!(table_node.children().contains(&header_row));
        assert!(table_node.children().contains(&beta_row));
        assert!(header_row_node.children().contains(&name_header));
        assert!(header_row_node.children().contains(&score_header));
        assert!(beta_row_node.children().contains(&beta_name_cell));
        assert!(beta_row_node.children().contains(&beta_score_cell));

        LogicalNodeIds {
            table,
            header_row,
            name_header,
            score_header,
            beta_row,
            beta_name_cell,
            beta_score_cell,
        }
    }

    let sort_actions = Rc::new(RefCell::new(Vec::new()));
    let row_activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| PinningA11yProbe {
        column_pinned: false,
        row_pinned: false,
        sort_actions: sort_actions.clone(),
        row_activations: row_activations.clone(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial logical table nodes should publish");
    let initial_ids = logical_node_ids(&initial);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: initial_ids.beta_row,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("focused table tree should publish")
            .focus,
        initial_ids.beta_row
    );

    view.update(cx, |probe, cx| {
        probe.column_pinned = true;
        cx.notify();
    });
    cx.run_until_parked();
    let column_pinned = cx
        .latest_accessibility_tree_update()
        .expect("column-pinned logical table nodes should publish");
    assert_eq!(
        logical_node_ids(&column_pinned),
        initial_ids,
        "column pinning must not replace logical header, row, or cell nodes"
    );
    assert_eq!(column_pinned.focus, initial_ids.beta_row);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: initial_ids.score_header,
        data: None,
    }));
    assert_eq!(sort_actions.borrow().len(), 1);
    assert_eq!(sort_actions.borrow()[0].column_id().as_str(), "score");
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: initial_ids.beta_row,
        data: None,
    }));
    cx.run_until_parked();

    view.update(cx, |probe, cx| {
        probe.row_pinned = true;
        cx.notify();
    });
    cx.run_until_parked();
    let row_pinned = cx
        .latest_accessibility_tree_update()
        .expect("row-pinned logical table nodes should publish");
    assert_eq!(
        logical_node_ids(&row_pinned),
        initial_ids,
        "row pinning must not replace logical row or descendant cell nodes"
    );
    assert_eq!(row_pinned.focus, initial_ids.beta_row);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: initial_ids.beta_row,
        data: None,
    }));
    assert_eq!(
        row_activations.borrow().as_slice(),
        &[table_source_row_identity("beta")]
    );
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
        .debug_bounds(&table_source_row_center_scroll_selector(
            "semantic-virtual-table",
            "row-0000",
        ))
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
        .debug_bounds(&table_source_cell_selector(
            "semantic-virtual-table",
            "row-0000",
            "name",
        ))
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
    let (metric_ten_id, metric_ten) = row_ten
        .children()
        .iter()
        .filter_map(|child_id| vertical.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(7))
        .map(|(id, node)| (*id, node))
        .expect("vertically recycled metric cell should publish");
    assert_eq!(metric_ten.value(), Some("70"));

    let horizontal_return_viewport = cx
        .debug_bounds(&table_source_row_center_scroll_selector(
            "semantic-virtual-table",
            "row-0010",
        ))
        .expect("visible recycled row should expose its center viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: horizontal_return_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(10_000.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let vertical_return_cell = cx
        .debug_bounds(&table_source_cell_selector(
            "semantic-virtual-table",
            "row-0010",
            "name",
        ))
        .expect("visible recycled row should retain its pinned cell");
    cx.simulate_event(ScrollWheelEvent {
        position: vertical_return_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(10_000.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let returned = cx
        .latest_accessibility_tree_update()
        .expect("returned virtual table window should publish");
    let (returned_table_id, returned_table) =
        node_with_role_and_label(&returned, accesskit::Role::Table, "Virtual release table");
    assert_eq!(returned_table_id, table_id);
    let (returned_row_zero_id, returned_row_zero) =
        node_with_role_and_row_index(&returned, accesskit::Role::Row, 2);
    assert_eq!(returned_row_zero_id, row_zero_id);
    assert!(returned_table.children().contains(&returned_row_zero_id));
    let (returned_metric_zero_id, returned_metric_zero) = returned_row_zero
        .children()
        .iter()
        .filter_map(|child_id| returned.nodes.iter().find(|(id, _)| id == child_id))
        .find(|(_, node)| node.role() == accesskit::Role::Cell && node.column_index() == Some(2))
        .map(|(id, node)| (*id, node))
        .expect("original center cell should return with its logical row");
    assert_eq!(returned_metric_zero_id, metric_zero_id);
    assert_eq!(returned_metric_zero.value(), Some("10"));
    assert!(
        !returned
            .nodes
            .iter()
            .any(|(id, _)| { *id == row_ten_id || *id == metric_ten_id || *id == metric_five_id }),
        "temporary row and horizontal-window nodes must retire after returning to the origin"
    );
}

#[open_gpui::test]
fn stale_occurrence_focus_falls_back_without_retargeting_an_equal_replacement(
    cx: &mut open_gpui::TestAppContext,
) {
    fn rows() -> [TableRow; 2] {
        [
            TableRow::new("duplicate").with_cell("name", "First occurrence"),
            TableRow::new("duplicate").with_cell("name", "Second occurrence"),
        ]
    }

    struct OccurrenceFocusProbe {
        state: TableState,
        initial_focus: TableRowIdentity,
    }

    impl Render for OccurrenceFocusProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(360.0)).h(px(140.0)).child(
                Table::new(
                    "occurrence-focus-table",
                    "Occurrence focus table",
                    self.state.clone(),
                )
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(72.0))
                .default_focused_row(self.initial_focus.clone()),
            )
        }
    }

    let state = TableState::new(rows())
        .with_columns([TableColumn::new("name", "Name")])
        .with_pagination(TablePagination::disabled());
    let initial_focus = TableRowIdentity::Source(
        state
            .source_row_identity_at("duplicate", 1)
            .expect("second occurrence should resolve in the initial source snapshot"),
    );
    let stale_focus = initial_focus.clone();
    let (view, cx) = cx.add_window_view(|_, _| OccurrenceFocusProbe {
        state,
        initial_focus,
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial occurrence focus tree should publish");
    let second_cell = cell_with_column_and_value(&initial, 1, "Second occurrence").0;
    let second_row = parent_with_role(&initial, second_cell, accesskit::Role::Row).0;
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: second_row,
        data: None,
    }));
    cx.run_until_parked();

    view.update(cx, |probe, cx| {
        probe.state = probe.state.clone().with_rows(rows());
        cx.notify();
    });
    cx.run_until_parked();

    let replacement = cx
        .latest_accessibility_tree_update()
        .expect("equal source replacement should publish a new snapshot");
    assert!(!replacement.nodes.iter().any(|(id, _)| *id == second_row));
    let first_cell = cell_with_column_and_value(&replacement, 1, "First occurrence").0;
    let first_row = parent_with_role(&replacement, first_cell, accesskit::Role::Row).0;
    assert_eq!(replacement.focus, first_row);

    let replacement_identity = cx.update(|_, cx| {
        TableRowIdentity::Source(
            view.read(cx)
                .state
                .source_row_identity_at("duplicate", 0)
                .expect("first replacement occurrence should resolve"),
        )
    });
    assert_ne!(replacement_identity, stale_focus);
    assert!(cx.debug_selector_is_focused(&TableDebugSelector::row(
        "occurrence-focus-table",
        &replacement_identity,
    )));
}

#[open_gpui::test]
fn duplicate_source_rows_publish_distinct_stable_accessibility_nodes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct DuplicateRowsA11yProbe {
        activations: Rc<RefCell<Vec<TableRowIdentity>>>,
        reversed: bool,
        pin_second: bool,
    }

    impl Render for DuplicateRowsA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let mut rows = vec![
                TableRow::new("duplicate")
                    .with_instance_id("first")
                    .with_cell("name", "First duplicate")
                    .with_cell("score", 10_usize),
                TableRow::new("duplicate")
                    .with_instance_id("second")
                    .with_cell("name", "Second duplicate")
                    .with_cell("score", 20_usize),
            ];
            if self.reversed {
                rows.reverse();
            }
            let mut state = TableState::new(rows)
                .with_columns([
                    TableColumn::new("name", "Name"),
                    TableColumn::new("score", "Score"),
                ])
                .with_pagination(TablePagination::disabled());
            if self.pin_second {
                state =
                    state
                        .with_row_pinning(TableRowPinning::new().pinned_top([
                            TableRowIdentity::source_instance("duplicate", "second"),
                        ]));
            }

            div().w(px(420.0)).h(px(180.0)).child(
                Table::new(
                    "duplicate-semantic-table",
                    "Duplicate semantic table",
                    state,
                )
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_row_activate(move |activation, _, _| {
                    activations.borrow_mut().push(activation.identity().clone());
                }),
            )
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DuplicateNodeIds {
        table: accesskit::NodeId,
        first_row: accesskit::NodeId,
        second_row: accesskit::NodeId,
        first_cell: accesskit::NodeId,
        second_cell: accesskit::NodeId,
    }

    fn duplicate_node_ids(update: &accesskit::TreeUpdate) -> DuplicateNodeIds {
        let table =
            node_with_role_and_label(update, accesskit::Role::Table, "Duplicate semantic table");
        assert_eq!(table.1.row_count(), Some(3));
        assert_eq!(table.1.column_count(), Some(2));
        let first_cell = cell_with_column_and_value(update, 1, "First duplicate").0;
        let second_cell = cell_with_column_and_value(update, 1, "Second duplicate").0;
        let first_row = parent_with_role(update, first_cell, accesskit::Role::Row).0;
        let second_row = parent_with_role(update, second_cell, accesskit::Role::Row).0;
        assert_ne!(first_row, second_row);
        assert_ne!(first_cell, second_cell);
        assert!(table.1.children().contains(&first_row));
        assert!(table.1.children().contains(&second_row));

        DuplicateNodeIds {
            table: table.0,
            first_row,
            second_row,
            first_cell,
            second_cell,
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| DuplicateRowsA11yProbe {
        activations: activations.clone(),
        reversed: false,
        pin_second: false,
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("duplicate-row accessibility tree should publish");
    let initial_ids = duplicate_node_ids(&initial);
    let unique_ids = initial
        .nodes
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique_ids.len(), initial.nodes.len());

    for target_node in [initial_ids.first_row, initial_ids.second_row] {
        assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node,
            data: None,
        }));
    }
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            TableRowIdentity::source_instance("duplicate", "first"),
            TableRowIdentity::source_instance("duplicate", "second"),
        ]
    );

    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let rerendered = cx
        .latest_accessibility_tree_update()
        .expect("rerendered duplicate-row accessibility tree should publish");
    assert_eq!(duplicate_node_ids(&rerendered), initial_ids);
    let rerendered_unique_ids = rerendered
        .nodes
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(rerendered_unique_ids.len(), rerendered.nodes.len());

    view.update(cx, |probe, cx| {
        probe.reversed = true;
        cx.notify();
    });
    cx.run_until_parked();
    let reordered = cx
        .latest_accessibility_tree_update()
        .expect("reordered duplicate-row accessibility tree should publish");
    assert_eq!(duplicate_node_ids(&reordered), initial_ids);

    view.update(cx, |probe, cx| {
        probe.pin_second = true;
        cx.notify();
    });
    cx.run_until_parked();
    let pinned = cx
        .latest_accessibility_tree_update()
        .expect("pinned duplicate-row accessibility tree should publish");
    assert_eq!(duplicate_node_ids(&pinned), initial_ids);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: initial_ids.second_row,
        data: None,
    }));
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            TableRowIdentity::source_instance("duplicate", "first"),
            TableRowIdentity::source_instance("duplicate", "second"),
            TableRowIdentity::source_instance("duplicate", "second"),
        ]
    );
}
