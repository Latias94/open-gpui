use super::*;

#[test]
fn table_behavior_snapshot_uses_core_state_and_virtualizer_contracts() {
    let state = sample_table_state(100)
        .with_sorting([TableSort::new("score", TableSortDirection::Descending)])
        .with_selected_rows(["row-0091"])
        .with_filters([TableFilter::contains("team", "UI")])
        .with_pagination(TablePagination::disabled());
    let table = Table::new("contracts-table", "Contracts", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .overscan(4);
    let snapshot = table.behavior_snapshot(ui_px(120.0), ui_px(96.0));

    assert_eq!(snapshot.role(), Role::Table);
    assert_eq!(snapshot.row_role(), Role::Row);
    assert_eq!(snapshot.column_header_role(), Role::ColumnHeader);
    assert_eq!(snapshot.cell_role(), Role::Cell);
    assert_eq!(snapshot.columns().len(), 3);
    assert_eq!(snapshot.aria_column_count(), 3);
    assert_eq!(snapshot.aria_row_count(), 51);
    assert_eq!(
        *snapshot.visible_rows().visible_range(),
        VirtualizerRange::new(5, 9)
    );
    assert_eq!(
        *snapshot.visible_rows().overscan_range(),
        VirtualizerRange::new(3, 11)
    );
    assert!(
        snapshot.row_counts().rendered_rows()
            <= snapshot.row_counts().visible_rows() + snapshot.metrics().overscan()
    );
    assert_eq!(snapshot.rows()[0].model_index(), 3);
    assert_eq!(snapshot.rows()[0].id().as_str(), "row-0093");
    assert_eq!(snapshot.rows()[0].role(), Role::Row);
    assert_eq!(snapshot.rows()[0].cells()[0].role(), Role::Cell);
    assert!(
        snapshot
            .rows()
            .iter()
            .any(|row| row.id().as_str() == "row-0091" && row.selected()),
        "expected selection to follow row id after filtering and sorting"
    );

    let score_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should be present");
    assert_eq!(
        score_column.sort_direction(),
        Some(TableSortDirection::Descending)
    );
    assert_eq!(score_column.accessible_label(), "Score, sorted descending");
}

#[test]
fn table_behavior_snapshot_exposes_tree_row_metadata_for_adapter_rendering() {
    let state = TableState::new([TableRow::new("root")
        .with_cell("name", "Workspace")
        .with_cell("status", "Ready")
        .with_child(
            TableRow::new("child")
                .with_cell("name", "UI")
                .with_cell("status", "Building"),
        )])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(160.0)),
        TableColumn::new("status", "Status").with_width(ui_px(120.0)),
    ])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_expanded_rows(["root"])
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("tree-table", "Tree table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.rows().len(), 2);
    assert_eq!(snapshot.rows()[0].id().as_str(), "root");
    assert!(snapshot.rows()[0].is_tree_branch());
    assert_eq!(snapshot.rows()[0].tree_expanded(), Some(true));
    assert_eq!(snapshot.rows()[0].depth(), 0);
    assert_eq!(snapshot.rows()[1].id().as_str(), "child");
    assert!(!snapshot.rows()[1].is_tree_branch());
    assert_eq!(snapshot.rows()[1].tree_expanded(), None);
    assert_eq!(snapshot.rows()[1].depth(), 1);
    assert_eq!(
        snapshot.rows()[0]
            .cells_for_region(TableColumnRegion::Left)
            .map(|cell| cell.column_id().as_str())
            .collect::<Vec<_>>(),
        ["name"]
    );
}

#[test]
fn table_behavior_snapshot_exposes_manual_expansion_and_child_load_metadata() {
    let manual_state = TableState::new([TableRow::new("root")
        .with_cell("name", "Workspace")
        .with_child(TableRow::new("child").with_cell("name", "Loaded child"))])
    .with_columns([TableColumn::new("name", "Name").with_width(ui_px(160.0))])
    .with_pagination(TablePagination::disabled());
    let manual_snapshot = Table::new("manual-tree", "Manual tree", manual_state)
        .expansion_mode(TableExpansionMode::Manual)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(
        manual_snapshot
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["root", "child"],
        "manual expansion should render the caller-supplied visible tree snapshot"
    );
    assert_eq!(manual_snapshot.rows()[0].tree_expanded(), Some(false));
    assert_eq!(manual_snapshot.rows()[0].loaded_child_count(), 1);
    assert_eq!(
        manual_snapshot.rows()[0].children_load_state(),
        Some(&TableRowChildrenLoadState::Idle)
    );

    let loading_state = TableState::new([TableRow::new("remote")
        .with_cell("name", "Remote branch")
        .with_children_loading("Loading children")])
    .with_columns([TableColumn::new("name", "Name").with_width(ui_px(160.0))])
    .with_pagination(TablePagination::disabled());
    let loading_snapshot = Table::new("loading-tree", "Loading tree", loading_state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let loading_row = &loading_snapshot.rows()[0];

    assert!(loading_row.is_tree_branch());
    assert_eq!(loading_row.loaded_child_count(), 0);
    assert_eq!(
        loading_row
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Loading children")
    );
    assert!(
        loading_row
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_loading)
    );
}

#[test]
fn table_behavior_snapshot_exposes_manual_row_model_metadata() {
    let state = TableState::new([
        TableRow::new("row-020")
            .with_cell("name", "Delta")
            .with_cell("team", "UI")
            .with_cell("score", 20_usize),
        TableRow::new("row-021")
            .with_cell("name", "Echo")
            .with_cell("team", "Platform")
            .with_cell("score", 21_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_width(ui_px(160.0)),
        TableColumn::new("team", "Team").with_width(ui_px(120.0)),
        TableColumn::new("score", "Score").with_width(ui_px(96.0)),
    ])
    .with_filters([TableFilter::contains("team", "missing")])
    .with_manual_filtering()
    .with_sorting([TableSort::ascending("score")])
    .with_manual_sorting()
    .with_pagination(TablePagination::manual(10, 2, 42));

    let snapshot = Table::new("manual-row-model", "Manual row model", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.filtering_mode(), TableStageMode::Manual);
    assert_eq!(snapshot.sorting_mode(), TableStageMode::Manual);
    assert_eq!(snapshot.pagination_mode(), TableStageMode::Manual);
    assert_eq!(snapshot.pagination_row_count(), Some(42));
    assert_eq!(snapshot.pagination_page_count(), Some(21));
    assert_eq!(
        snapshot
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-020", "row-021"],
        "manual row-model stages should render the caller-supplied page snapshot"
    );
}
