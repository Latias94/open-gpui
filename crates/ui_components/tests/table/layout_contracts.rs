use super::*;

#[test]
fn table_behavior_snapshot_exposes_pinned_column_regions() {
    let flat_snapshot = Table::new("flat-table", "Flat table", sample_table_state(1))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    assert_eq!(flat_snapshot.column_regions().left_columns(), 0);
    assert_eq!(flat_snapshot.column_regions().right_columns(), 0);

    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
        TableColumn::new("status", "Status"),
    ])
    .with_column_order(["status", "score", "team", "name"])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name", "score"])
            .pinned_right(["status"]),
    )
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("pinned-table", "Pinned table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let regions = snapshot.column_regions();
    assert_eq!(regions.left_width(), ui_px(256.0));
    assert_eq!(regions.center_width(), ui_px(128.0));
    assert_eq!(regions.right_width(), ui_px(128.0));
    assert_eq!(regions.total_width(), ui_px(512.0));

    let region_columns = snapshot
        .columns()
        .iter()
        .map(|column| (column.id().as_str(), column.region()))
        .collect::<Vec<_>>();
    assert_eq!(
        region_columns,
        [
            ("score", TableColumnRegion::Left),
            ("name", TableColumnRegion::Left),
            ("team", TableColumnRegion::Center),
            ("status", TableColumnRegion::Right),
        ]
    );

    let row = &snapshot.rows()[0];
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Left)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["score", "name"]
    );
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Center)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["team"]
    );
    assert_eq!(
        row.cells_for_region(TableColumnRegion::Right)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["status"]
    );
}

#[test]
fn table_behavior_snapshot_exposes_row_pinning_regions() {
    let state = sample_table_state(12)
        .with_pagination(TablePagination::new(1, 4))
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top(["row-0001"])
                .pinned_bottom(["row-0005", "row-0010"]),
        );
    let snapshot = Table::new("row-pinning-table", "Row pinning table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .overscan(0)
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(
        snapshot
            .rows_for_region(TableRowRegion::Top)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [("row-0001", TableRowRegion::Top, 0)]
    );
    assert_eq!(
        snapshot
            .rows_for_region(TableRowRegion::Center)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-0004", TableRowRegion::Center, 0),
            ("row-0006", TableRowRegion::Center, 1),
            ("row-0007", TableRowRegion::Center, 2),
        ],
        "the center region should be the current page with pinned duplicates removed"
    );
    assert_eq!(
        snapshot
            .rows_for_region(TableRowRegion::Bottom)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-0005", TableRowRegion::Bottom, 0),
            ("row-0010", TableRowRegion::Bottom, 1),
        ]
    );
    assert_eq!(
        snapshot
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "row-0001", "row-0004", "row-0006", "row-0007", "row-0005", "row-0010",
        ]
    );
    assert_eq!(snapshot.row_counts().pinned_center_rows(), 3);
    assert_eq!(snapshot.row_counts().rendered_rows(), 6);
    assert_eq!(snapshot.row_counts().visible_rows(), 6);
    assert_eq!(snapshot.aria_row_count(), 7);
}

#[test]
fn table_behavior_snapshot_respects_page_only_row_pinning_policy() {
    let state = sample_table_state(12)
        .with_pagination(TablePagination::new(1, 4))
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top(["row-0001"])
                .pinned_bottom(["row-0005", "row-0010"]),
        )
        .with_row_pinning_policy(TableRowPinningPolicy::PageOnly);
    let snapshot = Table::new(
        "row-pinning-page-only-table",
        "Row pinning page-only table",
        state,
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.row_counts().pinned_top_rows(), 0);
    assert_eq!(
        snapshot
            .rows_for_region(TableRowRegion::Center)
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-0004", "row-0006", "row-0007"],
        "outside-page pinned rows should be omitted under page-only policy"
    );
    assert_eq!(
        snapshot
            .rows_for_region(TableRowRegion::Bottom)
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-0005"]
    );
    assert_eq!(snapshot.row_counts().pinned_center_rows(), 3);
    assert_eq!(snapshot.aria_row_count(), 5);
}

#[test]
fn table_behavior_snapshot_exposes_center_column_summary_without_window_internals() {
    let snapshot = Table::new(
        "center-window-table",
        "Center window table",
        sample_center_window_table_state(),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(4)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    let regions = snapshot.column_regions();
    assert_eq!(regions.left_columns(), 1);
    assert_eq!(regions.center_columns(), 6);
    assert_eq!(regions.right_columns(), 1);
    assert_eq!(regions.left_width(), ui_px(140.0));
    assert_eq!(regions.center_width(), ui_px(540.0));
    assert_eq!(regions.right_width(), ui_px(132.0));
    assert_eq!(regions.aria_columns(), 8);
    assert!(
        snapshot
            .columns()
            .iter()
            .filter(|column| column.region() == TableColumnRegion::Center)
            .all(|column| column.id().as_str().starts_with("metric_"))
    );
}

#[test]
fn table_behavior_snapshot_keeps_virtualized_visible_range_stable_with_snapshot() {
    let snapshot = VirtualizerSnapshot::new(
        ui_px(0.0),
        [VirtualizerSnapshotItem::new(
            VirtualizerItemKey::new("row-0005"),
            ui_px(48.0),
        )],
    );
    let table = Table::new("snapshot-table", "Snapshot table", sample_table_state(30))
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .virtualizer_snapshot(snapshot);
    let snapshot = table.behavior_snapshot(ui_px(120.0), ui_px(96.0));

    assert_eq!(
        *snapshot.visible_rows().visible_range(),
        VirtualizerRange::new(5, 8)
    );
    assert_eq!(
        *snapshot.visible_rows().overscan_range(),
        VirtualizerRange::new(2, 11)
    );
    assert_eq!(snapshot.rows()[0].id().as_str(), "row-0002");
}

#[test]
fn table_behavior_snapshot_preserves_duplicate_row_id_visibility() {
    let state = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
        TableRow::new("unique").with_cell("name", "Third"),
    ])
    .with_columns([TableColumn::new("name", "Name")])
    .with_pagination(TablePagination::disabled());
    let table = Table::new("duplicate-table", "Duplicate rows", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(120.0));
    let snapshot = table.behavior_snapshot(UiPx::ZERO, ui_px(120.0));

    assert_eq!(
        snapshot
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["duplicate", "duplicate", "unique"]
    );
    assert_eq!(snapshot.row_counts().rendered_rows(), 3);
}

#[test]
fn table_behavior_snapshot_exposes_column_sizing_metadata_and_matching_cell_widths() {
    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("score", 42_usize)
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(100.0)),
        TableColumn::new("team", "Team").with_width(ui_px(120.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(80.0))
            .with_min_width(ui_px(70.0))
            .with_max_width(ui_px(90.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(60.0))
            .with_resizable(false),
    ])
    .with_column_order(["status", "score", "team", "name"])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name", "score"])
            .pinned_right(["status"]),
    )
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(95.0)))
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("sized-table", "Sized table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.column_regions().total_width(), ui_px(370.0));
    assert_eq!(snapshot.column_regions().left_width(), ui_px(190.0));
    assert_eq!(snapshot.column_regions().center_width(), ui_px(120.0));
    assert_eq!(snapshot.column_regions().right_width(), ui_px(60.0));

    let score_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should be present");
    assert_eq!(score_column.width(), ui_px(90.0));
    assert!(score_column.resizable());

    let score_cell = snapshot.rows()[0]
        .cells_for_region(TableColumnRegion::Left)
        .find(|cell| cell.column_id().as_str() == "score")
        .expect("score cell should be present");
    assert_eq!(score_cell.width(), score_column.width());
}

#[test]
fn table_behavior_snapshot_preserves_column_width_policies() {
    let state = TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("status", "Ready")])
    .with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("status", "Status").with_content_fit(),
    ])
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("policy-table", "Policy table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let status_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "status")
        .expect("status column should be present");

    assert_eq!(
        status_column.width_policy(),
        open_gpui_ui_core::TableColumnWidthPolicy::ContentFit
    );
}

#[test]
fn table_behavior_snapshot_exposes_nested_header_summary_and_region_widths() {
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
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
            ],
        ),
        TableColumnGroup::new(
            "metrics",
            "Metrics",
            [TableColumnGroup::new(
                "scores",
                "Scores",
                [
                    TableColumn::new("score", "Score").with_width(ui_px(80.0)),
                    TableColumn::new("status", "Status").with_width(ui_px(90.0)),
                ],
            )],
        ),
    ])
    .with_column_pinning(
        TableColumnPinning::new()
            .pinned_left(["name"])
            .pinned_right(["status"]),
    )
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("nested-headers", "Nested headers", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(240.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(240.0));

    assert_eq!(snapshot.header_summary().header_rows(), 3);
    assert_eq!(snapshot.header_summary().visible_group_headers(), 3);
    assert_eq!(snapshot.column_regions().left_width(), ui_px(100.0));
    assert_eq!(
        snapshot.column_regions().center_width(),
        ui_px(120.0 + 80.0)
    );
    assert_eq!(snapshot.column_regions().right_width(), ui_px(90.0));
}

#[test]
fn table_header_action_cycles_sorting_without_render_coupling() {
    let unsorted = Table::new("sort-cycle", "Sort cycle", sample_table_state(8))
        .behavior_snapshot(UiPx::ZERO, ui_px(120.0));
    let name_action = unsorted
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("sortable column should expose a header action");
    assert_eq!(name_action.current_direction(), None);
    assert_eq!(
        name_action.next_direction(),
        Some(TableSortDirection::Ascending)
    );

    let ascending_state = name_action.apply_to(sample_table_state(8));
    assert_eq!(ascending_state.sorting()[0].column().as_str(), "name");
    assert_eq!(
        ascending_state.sorting()[0].direction(),
        TableSortDirection::Ascending
    );

    let ascending = Table::new("sort-cycle", "Sort cycle", ascending_state)
        .behavior_snapshot(UiPx::ZERO, ui_px(120.0));
    let descending_action = ascending
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("ascending column should expose a descending action");
    assert_eq!(
        descending_action.current_direction(),
        Some(TableSortDirection::Ascending)
    );
    assert_eq!(
        descending_action.next_direction(),
        Some(TableSortDirection::Descending)
    );

    let descending_state = descending_action.apply_to(sample_table_state(8));
    let descending = Table::new("sort-cycle", "Sort cycle", descending_state)
        .behavior_snapshot(UiPx::ZERO, ui_px(120.0));
    let clear_action = descending
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .and_then(|column| column.sort_action())
        .expect("descending column should expose a clear action");
    assert_eq!(
        clear_action.current_direction(),
        Some(TableSortDirection::Descending)
    );
    assert_eq!(clear_action.next_direction(), None);
    assert!(
        clear_action
            .apply_to(sample_table_state(8))
            .sorting()
            .is_empty()
    );
}
