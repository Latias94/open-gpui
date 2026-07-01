mod support;

use open_gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, Window, div, point, px,
};
use open_gpui_ui_components::{
    PopoverOpenMode, ScrollArea, Table, TableCellEditApplyOutcome, TableCellEditChange,
    TableCellEditor, TableCellValue, TableColumn, TableColumnFacets, TableColumnGroup,
    TableColumnId, TableColumnOrderChange, TableColumnOrderPlacement, TableColumnPinning,
    TableColumnRegion, TableColumnResizeMode, TableColumnSizing, TableColumnSizingChange,
    TableColumnVisibility, TableColumnVisibilityAction, TableColumnVisibilityChange,
    TableColumnVisibilityOverrides, TableColumnVisibilityState, TableExpansionMode,
    TableFacetValueCount, TableFacetedFilter, TableFacetedFilterChange, TableFacetedFilterState,
    TableFilter, TableGlobalFacetSummary, TableGlobalFilter, TableGlobalFilterChange,
    TableGlobalFilterState, TableHeaderAction, TableNumericFilterOperator, TablePagination,
    TablePredicateFilter, TablePredicateFilterChange, TablePredicateFilterOperator,
    TablePredicateFilterOperatorOptionState, TablePredicateFilterState, TableRangeFilter,
    TableRangeFilterChange, TableRangeFilterState, TableRow, TableRowChildrenLoadState, TableRowId,
    TableRowMeasureMode, TableRowPinning, TableRowPinningPolicy, TableRowRegion, TableSelectOption,
    TableSelectionActivationMode, TableSelectionMode, TableSelectionScope, TableSort,
    TableSortDirection, TableStageMode, TableState, TableTextFilterOperator, TableToolbar,
    TableToolbarState, VirtualizerItemKey, VirtualizerRange, VirtualizerSnapshot,
    VirtualizerSnapshotItem, gpui_adapter::init_text_input,
};
use open_gpui_ui_core::{
    OutsidePressPolicy, OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size, UiPx,
    ui_px,
};
use std::cell::RefCell;
use std::rc::Rc;
use support::tokens::{TEST_TEXT, TEST_TEXT_MUTED, custom_tokens};

fn sample_table_state(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Package {index:04}"))
            .with_cell(
                "team",
                if index.is_multiple_of(2) {
                    "Core"
                } else {
                    "UI"
                },
            )
            .with_cell("score", index)
    });

    TableState::new(rows).with_columns([
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
}

fn text_facet_counts(facet: &TableColumnFacets) -> Vec<(String, usize)> {
    facet
        .unique_values()
        .iter()
        .map(|entry| match entry.value() {
            TableCellValue::Text(value) => (value.clone(), entry.count()),
            value => panic!("expected text facet value, got {value:?}"),
        })
        .collect()
}

fn sample_pinned_table_state() -> TableState {
    TableState::new([TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "Platform")
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
    .with_pagination(TablePagination::disabled())
}

fn sample_pinned_table_state_with_rows(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Alpha {index:04}"))
            .with_cell(
                "team",
                if index.is_multiple_of(2) {
                    "Platform"
                } else {
                    "UI"
                },
            )
            .with_cell("score", index + 1)
            .with_cell(
                "status",
                if index.is_multiple_of(3) {
                    "Ready"
                } else {
                    "Queued"
                },
            )
    });

    TableState::new(rows)
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
        .with_pagination(TablePagination::disabled())
}

fn sample_center_window_table_state() -> TableState {
    let row = TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("metric_00", 10_usize)
        .with_cell("metric_01", 20_usize)
        .with_cell("metric_02", 30_usize)
        .with_cell("metric_03", 40_usize)
        .with_cell("metric_04", 50_usize)
        .with_cell("metric_05", 60_usize)
        .with_cell("status", "Ready");

    sample_center_window_table_state_from_rows([row])
}

fn sample_center_window_table_state_with_rows(row_count: usize) -> TableState {
    let rows = (0..row_count).map(|index| {
        TableRow::new(format!("row-{index:04}"))
            .with_cell("name", format!("Package {index:04}"))
            .with_cell("metric_00", index + 10)
            .with_cell("metric_01", index + 20)
            .with_cell("metric_02", index + 30)
            .with_cell("metric_03", index + 40)
            .with_cell("metric_04", index + 50)
            .with_cell("metric_05", index + 60)
            .with_cell(
                "status",
                if index.is_multiple_of(2) {
                    "Ready"
                } else {
                    "Queued"
                },
            )
    });

    sample_center_window_table_state_from_rows(rows)
}

fn sample_center_window_table_state_from_rows(
    rows: impl IntoIterator<Item = TableRow>,
) -> TableState {
    TableState::new(rows)
        .with_columns([
            TableColumn::new("name", "Name").with_width(ui_px(140.0)),
            TableColumn::new("metric_00", "Metric 00").with_width(ui_px(60.0)),
            TableColumn::new("metric_01", "Metric 01").with_width(ui_px(72.0)),
            TableColumn::new("metric_02", "Metric 02").with_width(ui_px(84.0)),
            TableColumn::new("metric_03", "Metric 03").with_width(ui_px(96.0)),
            TableColumn::new("metric_04", "Metric 04").with_width(ui_px(108.0)),
            TableColumn::new("metric_05", "Metric 05").with_width(ui_px(120.0)),
            TableColumn::new("status", "Status").with_width(ui_px(132.0)),
        ])
        .with_column_order([
            "name",
            "metric_00",
            "metric_01",
            "metric_02",
            "metric_03",
            "metric_04",
            "metric_05",
            "status",
        ])
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_pagination(TablePagination::disabled())
}

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

#[test]
fn table_behavior_snapshot_exposes_faceting_metadata() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize),
        TableRow::new("row-4")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("status", "Ready"),
        TableFilter::contains("team", "UI"),
    ])
    .with_pagination(TablePagination::new(0, 1))
    .with_manual_facets(
        [TableColumnFacets::manual("status", 64).with_unique_values([
            TableFacetValueCount::new("Blocked", 24),
            TableFacetValueCount::new("Ready", 40),
        ])],
    );

    let snapshot = Table::new("faceted-table", "Faceted table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.faceting_mode(), TableStageMode::Client);
    assert_eq!(snapshot.column_facets().len(), 3);
    assert_eq!(
        snapshot
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-1"],
        "pagination still limits the rendered row window"
    );

    let status = snapshot
        .column_facet(&TableColumnId::new("status"))
        .expect("status facet should resolve");
    assert_eq!(status.mode(), TableStageMode::Manual);
    assert_eq!(status.row_count(), 64);
    assert_eq!(
        text_facet_counts(status),
        [("Blocked".to_string(), 24), ("Ready".to_string(), 40)],
        "manual facet payloads should survive render-plan resolution"
    );

    let team = snapshot
        .column_facet(&TableColumnId::new("team"))
        .expect("team facet should resolve");
    assert_eq!(team.mode(), TableStageMode::Client);
    assert_eq!(team.row_count(), 3);
    assert_eq!(
        text_facet_counts(team),
        [("API".to_string(), 1), ("UI".to_string(), 2)],
        "client facets ignore their own column filter and honor the other filters"
    );

    let score = snapshot
        .column_facet(&TableColumnId::new("score"))
        .expect("score facet should resolve");
    let range = score
        .numeric_range()
        .expect("score facet should expose a numeric range");
    assert_eq!(range.min(), 10.0);
    assert_eq!(range.max(), 40.0);
}

#[test]
fn table_behavior_snapshot_exposes_global_facet_summary() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "alpha")
            .with_cell("notes", "ready"),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize)
            .with_cell("enabled", false)
            .with_cell("notes", "done"),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "beta")
            .with_cell("notes", "done"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
        TableColumn::new("enabled", "Enabled"),
        TableColumn::new("tag", "Tag"),
        TableColumn::new("notes", "Notes").with_global_filterable(false),
    ])
    .with_filters([TableFilter::contains("team", "UI")])
    .with_global_filter("done")
    .with_pagination(TablePagination::disabled());

    let snapshot = Table::new("global-facet-table", "Global facet table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    let summary: &TableGlobalFacetSummary = snapshot.global_facet_summary();
    assert_eq!(summary.mode(), TableStageMode::Client);
    assert_eq!(summary.row_count(), 2);
    assert!(summary.column_facet(&TableColumnId::new("notes")).is_none());
    assert_eq!(
        summary
            .column_facets()
            .iter()
            .map(|facet| facet.column().as_str())
            .collect::<Vec<_>>(),
        ["team", "status", "score", "enabled", "tag"]
    );
    assert_eq!(
        text_facet_counts(
            summary
                .column_facet(&TableColumnId::new("status"))
                .expect("status global facet should resolve")
        ),
        [("Blocked".to_string(), 1), ("Ready".to_string(), 1)]
    );
}

#[test]
fn table_faceted_filter_state_resolves_query_selection_and_popover_contract() {
    let facets = TableColumnFacets::manual("status", 4).with_unique_values([
        TableFacetValueCount::new("Ready", 2),
        TableFacetValueCount::new("Blocked", 1),
        TableFacetValueCount::new("Review", 1),
    ]);

    let state: TableFacetedFilterState =
        TableFacetedFilter::new("status-filter", "Status", "status")
            .facets(facets)
            .selected_values(["Ready", "Blocked"])
            .query("rea")
            .open(true)
            .placeholder("Find status")
            .empty_label("No statuses")
            .clear_label("Reset")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "status-filter");
    assert_eq!(state.label(), "Status");
    assert_eq!(state.column_id().as_str(), "status");
    assert_eq!(state.query(), "rea");
    assert_eq!(state.trigger_label(), "Status: Ready, Blocked");
    assert_eq!(
        state.selected_values(),
        &["Blocked".to_string(), "Ready".to_string()]
    );
    assert_eq!(
        state.selected_labels(),
        &["Ready".to_string(), "Blocked".to_string()]
    );
    assert_eq!(state.total_option_count(), 3);
    assert!(state.clear_enabled());
    assert_eq!(state.empty_label(), "No statuses");
    assert_eq!(state.clear_label(), "Reset");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.search_input().value(), "rea");
    assert_eq!(state.search_input().placeholder(), Some("Find status"));
    assert_eq!(state.search_input().size(), Size::Small);
    assert!(state.search_input().controller_driven());

    let options = state
        .options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.count(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![("Ready".to_string(), "Ready".to_string(), 2, true)]
    );
}

#[test]
fn table_faceted_filter_state_reports_empty_query_result() {
    let state = TableFacetedFilter::new("status-filter", "Status", "status")
        .facets(TableColumnFacets::manual("status", 2).with_unique_values([
            TableFacetValueCount::new("Ready", 1),
            TableFacetValueCount::new("Blocked", 1),
        ]))
        .query("missing")
        .empty_label("No matching statuses")
        .state();

    assert!(state.empty());
    assert!(!state.clear_enabled());
    assert_eq!(state.total_option_count(), 2);
    assert_eq!(state.empty_label(), "No matching statuses");
    assert!(state.options().is_empty());
}

#[test]
fn table_faceted_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready"),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change =
        TableFacetedFilterChange::new("status", ["Blocked", "Ready"], Some("Blocked"), true);
    assert_eq!(change.column_id().as_str(), "status");
    assert_eq!(change.toggled_value(), Some("Blocked"));
    assert!(change.selected());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 2);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let status_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "status")
        .expect("status filter should be replaced by the faceted selection");
    assert_eq!(
        status_filter
            .selected_values()
            .expect("status filter should be categorical")
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["Blocked".to_string(), "Ready".to_string()]
    );

    let cleared = TableFacetedFilterChange::clear("status");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 1);
    assert_eq!(cleared_state.filters()[0].column().as_str(), "team");
}

#[test]
fn table_range_filter_state_resolves_bounds_and_popover_contract() {
    let facets = TableColumnFacets::manual("score", 64).with_numeric_range(1.0, 64.0);

    let state: TableRangeFilterState = TableRangeFilter::new("score-range", "Score", "score")
        .facets(facets)
        .range(Some(40.0), Some(10.0))
        .open(true)
        .clear_label("Reset score")
        .small()
        .placement_side(OverlayPlacementSide::Top)
        .placement_alignment(OverlayPlacementAlignment::End)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .state();

    assert_eq!(state.id(), "score-range");
    assert_eq!(state.label(), "Score");
    assert_eq!(state.column_id().as_str(), "score");
    assert_eq!(state.min_text(), "10");
    assert_eq!(state.max_text(), "40");
    assert_eq!(state.min_value(), Some(10.0));
    assert_eq!(state.max_value(), Some(40.0));
    assert_eq!(state.trigger_label(), "Score: 10-40");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.clear_label(), "Reset score");
    let facet_range = state
        .facet_range()
        .expect("manual score facets should expose a numeric range");
    assert_eq!(facet_range.min(), 1.0);
    assert_eq!(facet_range.max(), 64.0);
    assert_eq!(state.min_placeholder(), "Min (1)");
    assert_eq!(state.max_placeholder(), "Max (64)");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.min_input().value(), "10");
    assert_eq!(state.max_input().value(), "40");
    assert_eq!(state.min_input().placeholder(), Some("Min (1)"));
    assert_eq!(state.max_input().placeholder(), Some("Max (64)"));
    assert!(state.min_input().controller_driven());
    assert!(state.max_input().controller_driven());
}

#[test]
fn table_range_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        TableFilter::number_range("score", Some(5.0), Some(20.0))
            .expect("initial score range should be valid"),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change = TableRangeFilterChange::new("score", "30", "10");
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.min_text(), "30");
    assert_eq!(change.max_text(), "10");
    assert_eq!(change.min_value(), Some(10.0));
    assert_eq!(change.max_value(), Some(30.0));
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 3);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let score_text_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "score" && filter.query() == "1")
        .expect("same-column non-range filter should be preserved");
    assert_eq!(score_text_filter.number_range_bounds(), None);
    let score_filter = next
        .filters()
        .iter()
        .find(|filter| {
            filter.column().as_str() == "score" && filter.number_range_bounds().is_some()
        })
        .expect("score filter should be replaced by the range selection");
    assert_eq!(
        score_filter.number_range_bounds(),
        Some((Some(10.0), Some(30.0)))
    );

    let cleared = TableRangeFilterChange::clear("score");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 2);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_range_bounds().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team")
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "score" && filter.query() == "1")
    );
}

#[test]
fn table_column_visibility_state_resolves_items_counts_and_popover_contract() {
    let visibility = TableColumnVisibilityOverrides::new()
        .hide("name")
        .show("team")
        .hide("score");

    let state: TableColumnVisibilityState =
        TableColumnVisibility::new("column-visibility", "Columns")
            .columns([
                TableColumn::new("name", "Name").with_hideable(false),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .visibility(visibility)
            .open(true)
            .empty_label("No columns configured")
            .show_all_label("Show every column")
            .reset_label("Reset columns")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "column-visibility");
    assert_eq!(state.label(), "Columns");
    assert_eq!(state.trigger_label(), "Columns: 1 hidden");
    assert_eq!(state.item_count(), 3);
    assert_eq!(state.visible_count(), 2);
    assert_eq!(state.hidden_count(), 1);
    assert_eq!(state.hideable_count(), 2);
    assert!(!state.all_visible());
    assert!(state.some_visible());
    assert!(state.show_all_enabled());
    assert!(state.reset_enabled());
    assert_eq!(state.empty_label(), "No columns configured");
    assert_eq!(state.show_all_label(), "Show every column");
    assert_eq!(state.reset_label(), "Reset columns");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );

    let items = state
        .items()
        .iter()
        .map(|item| {
            (
                item.column_id().as_str().to_owned(),
                item.label().to_owned(),
                item.checked(),
                item.hideable(),
                item.disabled(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        items,
        vec![
            ("name".to_string(), "Name".to_string(), true, false, true),
            ("team".to_string(), "Team".to_string(), true, true, false),
            ("score".to_string(), "Score".to_string(), false, true, false),
        ]
    );
}

#[test]
fn table_column_visibility_change_updates_visibility_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("name", "Beta")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_hideable(false),
        TableColumn::new("team", "Team").with_visible(false),
        TableColumn::new("score", "Score"),
    ])
    .with_column_visibility(
        TableColumnVisibilityOverrides::new()
            .hide("name")
            .show("team")
            .hide("score"),
    )
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::number_range("score", Some(5.0), Some(50.0))
            .expect("finite score range should be valid"),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_pagination(TablePagination::new(2, 25));

    let change = TableColumnVisibilityChange::new("team", false);
    assert_eq!(change.action(), TableColumnVisibilityAction::ToggleColumn);
    assert_eq!(change.action().as_str(), "toggle_column");
    assert_eq!(change.column_id().map(TableColumnId::as_str), Some("team"));
    assert_eq!(change.column_ids(), &[TableColumnId::new("team")]);
    assert_eq!(change.next_visible(), Some(false));

    let next = change.apply_to(state.clone());
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.pagination(), state.pagination());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());

    let show_all = TableColumnVisibilityChange::show_all(["team", "score"]);
    assert_eq!(show_all.action(), TableColumnVisibilityAction::ShowAll);
    assert_eq!(show_all.action().as_str(), "show_all");
    assert_eq!(show_all.next_visible(), Some(true));
    assert_eq!(show_all.column_ids().len(), 2);
    let shown = show_all.apply_to(state.clone());
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );

    let reset = TableColumnVisibilityChange::reset();
    assert_eq!(reset.action(), TableColumnVisibilityAction::Reset);
    assert_eq!(reset.action().as_str(), "reset");
    assert!(reset.column_ids().is_empty());
    assert_eq!(reset.column_id(), None);
    assert_eq!(reset.next_visible(), None);
    let reset_state = reset.apply_to(state);
    assert!(reset_state.column_visibility().is_empty());
}

#[test]
fn table_global_filter_state_resolves_input_contract() {
    let state: TableGlobalFilterState = TableGlobalFilter::new("global-filter", "Search rows")
        .default_query("stale")
        .query("  done  ")
        .placeholder("Search every row")
        .clear_label("Reset search")
        .small()
        .state();

    assert_eq!(state.id(), "global-filter");
    assert_eq!(state.label(), "Search rows");
    assert_eq!(state.query(), "  done  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Search every row");
    assert_eq!(state.clear_label(), "Reset search");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  done  ");
    assert_eq!(state.input().placeholder(), Some("Search every row"));
    assert_eq!(state.input().size(), Size::Small);
    assert!(state.input().controller_driven());

    let empty = TableGlobalFilter::new("empty-global-filter", "Search")
        .default_query("   ")
        .disabled(true)
        .state();
    assert!(!empty.active());
    assert!(empty.clear_enabled());
    assert!(empty.disabled());
    assert!(empty.input().disabled());
}

#[test]
fn table_toolbar_state_resolves_slot_counts_and_summary() {
    let tokens = custom_tokens();
    let state: TableToolbarState = TableToolbar::new("table-toolbar", "Filters")
        .small()
        .tokens(tokens)
        .control(div())
        .controls(vec![div(), div()])
        .secondary_control(div())
        .secondary_controls(vec![div(), div()])
        .summary("3 filtered / 8 total")
        .state();

    assert_eq!(state.id(), "table-toolbar");
    assert_eq!(state.label(), "Filters");
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.primary_control_count(), 3);
    assert_eq!(state.secondary_control_count(), 3);
    assert_eq!(state.control_count(), 6);
    assert!(state.has_controls());
    assert_eq!(state.summary(), Some("3 filtered / 8 total"));
    assert!(state.has_summary());
    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.tokens(), tokens);
    assert_eq!(state.foreground().token(), TEST_TEXT);
    assert_eq!(state.muted_foreground().token(), TEST_TEXT_MUTED);
    assert_eq!(state.colors().foreground().token(), TEST_TEXT);
    assert_eq!(state.colors().muted_foreground().token(), TEST_TEXT_MUTED);

    let empty = TableToolbar::new("empty-table-toolbar", "Filters").state();
    assert_eq!(empty.primary_control_count(), 0);
    assert_eq!(empty.secondary_control_count(), 0);
    assert_eq!(empty.control_count(), 0);
    assert!(!empty.has_controls());
    assert_eq!(empty.summary(), None);
    assert!(!empty.has_summary());
}

#[test]
fn table_global_filter_change_updates_state_and_resets_pagination() {
    let state = sample_table_state(4)
        .with_filters([TableFilter::contains("team", "UI")])
        .with_sorting([TableSort::ascending("name")])
        .with_selection_mode(TableSelectionMode::Multiple)
        .with_selected_rows(["row-0001"])
        .with_global_filter("old")
        .with_pagination(TablePagination::new(3, 25));

    let change = TableGlobalFilterChange::new("  done  ");
    assert_eq!(change.query(), "  done  ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.global_filter(), Some("done"));
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());

    let cleared = TableGlobalFilterChange::clear();
    assert_eq!(cleared.query(), "");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.global_filter(), None);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters(), state.filters());
    assert_eq!(cleared_state.sorting(), state.sorting());
    assert_eq!(cleared_state.selected_rows(), state.selected_rows());
}

#[test]
fn table_predicate_filter_state_resolves_operator_and_input_contract() {
    let starts_with = TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith);
    let state: TablePredicateFilterState =
        TablePredicateFilter::new("name-predicate", "Name", "name")
            .default_operator(TablePredicateFilterOperator::text(
                TableTextFilterOperator::Contains,
            ))
            .operator(starts_with)
            .default_value("stale")
            .value("  Al  ")
            .operators([
                TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith),
                TablePredicateFilterOperator::text(TableTextFilterOperator::EndsWith),
                TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThan),
            ])
            .placeholder("Filter name")
            .clear_label("Reset name")
            .small()
            .state();

    assert_eq!(state.id(), "name-predicate");
    assert_eq!(state.label(), "Name");
    assert_eq!(state.column_id().as_str(), "name");
    assert_eq!(state.operator(), starts_with);
    assert_eq!(
        state.operator().text_operator(),
        Some(TableTextFilterOperator::StartsWith)
    );
    assert_eq!(state.value(), "  Al  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Filter name");
    assert_eq!(state.clear_label(), "Reset name");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  Al  ");
    assert_eq!(state.input().placeholder(), Some("Filter name"));
    assert!(state.input().controller_driven());
    assert_eq!(state.select().selected_value(), Some("text:starts_with"));
    assert_eq!(state.select().trigger_label(), "Starts with");
    let first_option: &TablePredicateFilterOperatorOptionState = state
        .operator_options()
        .first()
        .expect("predicate filter should expose operator options");
    assert_eq!(first_option.operator(), starts_with);

    let options = state
        .operator_options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![
            (
                "text:starts_with".to_string(),
                "Starts with".to_string(),
                true,
            ),
            ("text:ends_with".to_string(), "Ends with".to_string(), false),
            (
                "number:greater_than".to_string(),
                "Greater than".to_string(),
                false,
            ),
        ]
    );

    let invalid_number = TablePredicateFilter::new("score-predicate", "Score", "score")
        .operator(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::GreaterThan,
        ))
        .value("not a number")
        .state();
    assert!(!invalid_number.active());
    assert!(invalid_number.clear_enabled());
    assert_eq!(
        TablePredicateFilterOperator::from_str("number:less_than")
            .expect("stable numeric operator should parse")
            .numeric_operator(),
        Some(TableNumericFilterOperator::LessThan)
    );
}

#[test]
fn table_predicate_filter_change_updates_only_target_predicate_filters() {
    let score_range = TableFilter::number_range("score", Some(0.0), Some(100.0))
        .expect("finite score range should be valid");
    let score_comparison = TableFilter::number_greater_than("score", 5.0)
        .expect("finite score comparison should be valid");
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked")
            .with_cell("score", 50_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        score_comparison,
        score_range.clone(),
        TableFilter::one_of("score", ["10", "50"]),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selection_mode(TableSelectionMode::Multiple)
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["team"]))
    .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_column_visibility(TableColumnVisibilityOverrides::new().hide("status"))
    .with_global_filter("ready")
    .with_pagination(TablePagination::new(3, 25));

    let change = TablePredicateFilterChange::new(
        "score",
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThanOrEqual),
        " 42 ",
    );
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(
        change.operator(),
        Some(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::LessThanOrEqual
        ))
    );
    assert_eq!(change.value(), " 42 ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());
    assert_eq!(next.column_visibility(), state.column_visibility());
    assert_eq!(next.global_filter(), state.global_filter());
    assert_eq!(next.filters().len(), 5);
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team" && filter.query() == "UI")
    );
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(next.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
    let score_predicate = next
        .filters()
        .iter()
        .find(|filter| filter.number_comparison_value().is_some())
        .expect("score numeric comparison should be replaced");
    assert_eq!(score_predicate.column().as_str(), "score");
    assert_eq!(
        score_predicate.number_comparison_value(),
        Some((TableNumericFilterOperator::LessThanOrEqual, 42.0))
    );
    assert!(
        next.filters().iter().all(|filter| {
            filter.column().as_str() != "score" || filter.text_predicate().is_none()
        }),
        "same-column legacy/text predicate should be removed"
    );

    let cleared = TablePredicateFilterChange::clear("score");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 4);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_comparison_value().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(cleared_state.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
}

#[test]
fn table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("notes", "Line 1\nLine 2")
            .with_cell("enabled", true)
            .with_cell("status", "ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-b").with_cell("score", 20_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("notes", "Notes").with_multiline_text_editor(3),
        TableColumn::new("enabled", "Enabled").with_checkbox_editor(),
        TableColumn::new("status", "Status")
            .with_select_editor([
                TableSelectOption::new("ready", "Ready"),
                TableSelectOption::new("blocked", "Blocked"),
            ])
            .with_width(ui_px(120.0)),
        TableColumn::new("score", "Score"),
    ])
    .with_grouping(["score"])
    .with_all_rows_expanded()
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("editable-plan-table", "Editable plan table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(120.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(120.0));

    let name_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .expect("name column should resolve");
    let score_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should resolve");
    let notes_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "notes")
        .expect("notes column should resolve");
    let enabled_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "enabled")
        .expect("enabled column should resolve");
    let status_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "status")
        .expect("status column should resolve");
    assert!(name_column.text_editable());
    assert_eq!(name_column.editor(), Some(TableCellEditor::Text));
    assert!(notes_column.text_editable());
    assert_eq!(
        notes_column.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert!(enabled_column.text_editable());
    assert_eq!(enabled_column.editor(), Some(TableCellEditor::Checkbox));
    assert_eq!(status_column.editor(), Some(TableCellEditor::Select));
    assert_eq!(status_column.select_options().len(), 2);
    assert_eq!(status_column.select_options()[0].value(), "ready");
    assert_eq!(status_column.select_options()[0].label(), "Ready");
    assert!(!score_column.text_editable());
    assert_eq!(score_column.editor(), None);

    let group_row = snapshot
        .rows()
        .iter()
        .find(|row| row.is_group())
        .expect("group row should resolve");
    let group_name_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("group name cell should resolve");
    assert!(
        !group_name_cell.text_editable(),
        "synthetic grouped rows must stay display-only"
    );
    let group_notes_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("group notes cell should resolve");
    assert_eq!(group_notes_cell.editor(), None);
    let group_enabled_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("group enabled cell should resolve");
    assert_eq!(group_enabled_cell.editor(), None);
    let group_status_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("group status cell should resolve");
    assert_eq!(group_status_cell.editor(), None);

    let editable_leaf = snapshot
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "row-a")
        .expect("row-a should resolve");
    let editable_name = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-a name cell should resolve");
    assert!(editable_name.text_editable());
    let editable_notes = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("row-a notes cell should resolve");
    assert_eq!(
        editable_notes.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    let editable_enabled = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("row-a enabled cell should resolve");
    assert_eq!(editable_enabled.editor(), Some(TableCellEditor::Checkbox));
    assert_eq!(editable_enabled.value(), Some(&TableCellValue::Bool(true)));
    let editable_status = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("row-a status cell should resolve");
    assert_eq!(editable_status.editor(), Some(TableCellEditor::Select));
    assert_eq!(editable_status.text(), "Ready");
    assert_eq!(editable_status.select_options().len(), 2);
    assert_eq!(editable_status.select_options()[1].value(), "blocked");

    let missing_leaf = snapshot
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "row-b")
        .expect("row-b should resolve");
    let missing_name = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-b missing name cell should resolve");
    assert!(!missing_name.text_editable());
    let missing_enabled = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("row-b missing enabled cell should resolve");
    assert_eq!(missing_enabled.editor(), None);
    let missing_status = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("row-b missing status cell should resolve");
    assert_eq!(missing_status.editor(), None);
}

#[test]
fn table_cell_edit_change_updates_source_row_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("name", "Root")
            .with_cell("team", "Platform")
            .with_child(
                TableRow::new("child")
                    .with_cell("name", "Child")
                    .with_cell("team", "UI"),
            ),
        TableRow::new("other")
            .with_cell("name", "Other")
            .with_cell("team", "Ops"),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("team", "Team"),
    ])
    .with_column_order(["team", "name"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_filters([TableFilter::contains("team", "UI")])
    .with_sorting([TableSort::ascending("name")])
    .with_expanded_rows(["root"])
    .with_selected_rows(["child"])
    .with_pagination(TablePagination::new(2, 25));

    let change = TableCellEditChange::for_row("child", "name", "Child", "Child Prime");

    let (next, outcome) = change.apply_to(state.clone());
    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(next.column_order()[0].as_str(), "team");
    assert_eq!(next.column_pinning().left()[0].as_str(), "name");
    assert_eq!(next.filters()[0].query(), "UI");
    assert_eq!(next.sorting()[0].column().as_str(), "name");
    assert_eq!(next.expansion(), state.expansion());
    assert!(next.selected_rows().contains(&TableRowId::new("child")));
    assert_eq!(next.pagination().page_index(), 2);

    let updated = next
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "root")
        .and_then(|row| row.children().first())
        .expect("nested child should remain nested");
    assert_eq!(
        updated
            .cell(&TableColumnId::new("name"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Child Prime")
    );

    let missing_column = TableCellEditChange::for_row("child", "missing", "old", "new");
    let (missing_column_state, missing_outcome) = missing_column.apply_to(next.clone());
    assert_eq!(missing_outcome, TableCellEditApplyOutcome::CellNotFound);
    assert_eq!(missing_column_state, next);
    assert_eq!(
        missing_column_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing cell edits should be inspectable no-ops"
    );

    let missing_row = TableCellEditChange::for_row("missing-row", "name", "old", "new");
    let (missing_row_state, missing_row_outcome) = missing_row.apply_to(next.clone());
    assert_eq!(missing_row_outcome, TableCellEditApplyOutcome::RowNotFound);
    assert_eq!(missing_row_state, next);
    assert_eq!(
        missing_row_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing row edits should be inspectable no-ops"
    );
}

#[test]
fn table_cell_edit_change_updates_boolean_source_row_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("name", "Root")
            .with_cell("team", "Platform")
            .with_cell("enabled", true)
            .with_child(
                TableRow::new("child")
                    .with_cell("name", "Child")
                    .with_cell("team", "UI")
                    .with_cell("enabled", true),
            ),
        TableRow::new("other")
            .with_cell("name", "Other")
            .with_cell("team", "Ops")
            .with_cell("enabled", false),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("team", "Team"),
        TableColumn::new("enabled", "Enabled").with_checkbox_editor(),
    ])
    .with_column_order(["team", "enabled", "name"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_filters([TableFilter::contains("team", "UI")])
    .with_sorting([TableSort::ascending("name")])
    .with_expanded_rows(["root"])
    .with_selected_rows(["child"])
    .with_pagination(TablePagination::new(2, 25));

    let change = TableCellEditChange::for_row("child", "enabled", true, false);

    let (next, outcome) = change.apply_to(state.clone());
    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(change.previous_value(), &TableCellValue::Bool(true));
    assert_eq!(change.next_value(), &TableCellValue::Bool(false));
    assert_eq!(change.previous_text(), "true");
    assert_eq!(change.next_text(), "false");
    assert_eq!(next.column_order()[0].as_str(), "team");
    assert_eq!(next.column_pinning().left()[0].as_str(), "name");
    assert_eq!(next.filters()[0].query(), "UI");
    assert_eq!(next.sorting()[0].column().as_str(), "name");
    assert_eq!(next.expansion(), state.expansion());
    assert!(next.selected_rows().contains(&TableRowId::new("child")));
    assert_eq!(next.pagination().page_index(), 2);

    let updated = next
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "root")
        .and_then(|row| row.children().first())
        .expect("nested child should remain nested");
    assert_eq!(
        updated.cell(&TableColumnId::new("enabled")),
        Some(&TableCellValue::Bool(false))
    );

    let missing_column = TableCellEditChange::for_row("child", "missing", true, false);
    let (missing_column_state, missing_outcome) = missing_column.apply_to(next.clone());
    assert_eq!(missing_outcome, TableCellEditApplyOutcome::CellNotFound);
    assert_eq!(missing_column_state, next);

    let missing_row = TableCellEditChange::for_row("missing-row", "enabled", true, false);
    let (missing_row_state, missing_row_outcome) = missing_row.apply_to(next.clone());
    assert_eq!(missing_row_outcome, TableCellEditApplyOutcome::RowNotFound);
    assert_eq!(missing_row_state, next);
}

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
        open_gpui_ui_components::TableColumnWidthPolicy::ContentFit
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

#[test]
fn table_public_exports_include_core_table_and_virtualizer_contracts() {
    use open_gpui_ui_components::{self as root, prelude};

    let state: root::TableState =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_columns([root::TableColumn::new("name", "Name")]);
    let table: root::Table = root::Table::new("root-table", "Root table", state.clone());
    let _prelude_state: prelude::TableState = state;
    let _prelude_table: prelude::Table = prelude::Table::new(
        "prelude-table",
        "Prelude table",
        root::TableState::new([root::TableRow::new("row-b").with_cell("name", "Beta")])
            .with_columns([root::TableColumn::new("name", "Name")]),
    );
    let virtualizer: root::VirtualizerState =
        root::VirtualizerState::new(4, ui_px(24.0)).with_overscan(2);
    let root_state_readout: &root::TableState = table.state();
    let root_resolved_state = root_state_readout.resolve();
    assert_eq!(root_resolved_state.final_model().rows().len(), 1);
    let root_snapshot: root::TableBehaviorSnapshot =
        table.behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let _prelude_snapshot: prelude::TableBehaviorSnapshot = root_snapshot.clone();
    let _root_region_snapshot: root::TableColumnRegionSnapshot = root_snapshot.column_regions();
    let _root_header_groups: &root::TableResolvedHeaderGroupRegions =
        root_resolved_state.header_groups();
    let _root_header_kind: root::TableResolvedHeaderKind =
        root_resolved_state.center_header_groups()[0].headers()[0].kind();
    let _root_header_cell: &root::TableResolvedHeaderCell =
        &root_resolved_state.center_header_groups()[0].headers()[0];
    let _root_header_group: &root::TableResolvedHeaderGroup =
        &root_resolved_state.center_header_groups()[0];
    let _root_header_summary: root::TableHeaderSummarySnapshot = root_snapshot.header_summary();
    let root_group_id = root::TableColumnGroupId::new("identity");
    assert_eq!(root_group_id.as_str(), "identity");
    let root_column_group = root::TableColumnGroup::new(
        root_group_id.clone(),
        "Identity",
        [root::TableColumn::new("name", "Name")],
    )
    .with_child(root::TableColumn::new("team", "Team"));
    let root_column_tree_state =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_column_tree([root_column_group.clone()]);
    let _root_column_node: &root::TableColumnNode = &root_column_tree_state.column_tree()[0];
    let _root_column_group: root::TableColumnGroup = root_column_group;
    let prelude_group = prelude::TableColumnGroup::new(
        prelude::TableColumnGroupId::new("status-group"),
        "Status",
        [prelude::TableColumn::new("status", "Status")],
    );
    let prelude_state =
        prelude::TableState::new([prelude::TableRow::new("row-c").with_cell("status", "Ready")])
            .with_column_tree([prelude::TableColumnNode::from(prelude_group)]);
    assert_eq!(prelude_state.columns()[0].id().as_str(), "status");
    let root_pinned_state = root::TableState::new([root::TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("status", "Ready")])
    .with_columns([
        root::TableColumn::new("name", "Name"),
        root::TableColumn::new("team", "Team"),
        root::TableColumn::new("status", "Status"),
    ])
    .with_column_pinning(
        root::TableColumnPinning::new()
            .pinned_left(["name"])
            .pinned_right(["status"]),
    );
    let root_pinned_snapshot =
        root::Table::new("root-pinned-table", "Root pinned table", root_pinned_state)
            .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let root_pinned_regions: root::TableColumnRegionSnapshot =
        root_pinned_snapshot.column_regions();
    let _prelude_pinned_regions: prelude::TableColumnRegionSnapshot = root_pinned_regions;
    let _prelude_header_summary: prelude::TableHeaderSummarySnapshot = _root_header_summary;
    assert_eq!(root_pinned_snapshot.table_id(), "root-pinned-table");
    let root_row_pinning: root::TableRowPinning = root::TableRowPinning::new()
        .pinned_top(["row-a"])
        .pinned_bottom(["row-b"]);
    let _prelude_row_pinning: prelude::TableRowPinning = root_row_pinning.clone();
    let _root_row_measure_mode: root::TableRowMeasureMode = root::TableRowMeasureMode::Measured;
    let _prelude_row_measure_mode: prelude::TableRowMeasureMode =
        prelude::TableRowMeasureMode::Fixed;
    let _root_row_pinning_policy: root::TableRowPinningPolicy =
        root::TableRowPinningPolicy::PageOnly;
    let _prelude_row_pinning_policy: prelude::TableRowPinningPolicy =
        prelude::TableRowPinningPolicy::KeepPinnedRows;
    let _root_row_region: root::TableRowRegion = root::TableRowRegion::Top;
    let _prelude_row_region: prelude::TableRowRegion = prelude::TableRowRegion::Bottom;
    let root_row_counts: root::TableRowCountSnapshot = root::Table::new(
        "root-row-pinning-table",
        "Root row pinning table",
        root::TableState::new([
            root::TableRow::new("row-a").with_cell("name", "Alpha"),
            root::TableRow::new("row-b").with_cell("name", "Beta"),
        ])
        .with_columns([root::TableColumn::new("name", "Name")])
        .with_row_pinning(root_row_pinning.clone()),
    )
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0))
    .row_counts();
    let _prelude_row_counts: prelude::TableRowCountSnapshot = root_row_counts;
    assert_eq!(root_pinned_regions.center_columns(), 1);
    let root_grid_viewport: root::GridViewport2D = root::resolve_grid_viewport_2d(
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
    );
    let _prelude_grid_viewport: prelude::GridViewport2D = root_grid_viewport.clone();
    let _prelude_grid_viewport_via_prelude: prelude::GridViewport2D =
        prelude::resolve_grid_viewport_2d(
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
        );
    assert_eq!(root_grid_viewport.row_overscan_range().start(), 0);
    let header_action: root::TableHeaderAction = root_snapshot.columns()[0]
        .sort_action()
        .expect("sortable exported table column should expose a header action")
        .clone();
    let _root_cache_key: root::TableStateCacheKey = table.state().cache_key();
    let _prelude_header_action: prelude::TableHeaderAction = header_action;
    let _prelude_cache_key: prelude::TableStateCacheKey = table.state().cache_key();
    let _root_aggregation: root::TableAggregation =
        root::TableAggregation::new("score", root::TableAggregateKind::Sum);
    let _prelude_aggregation: prelude::TableAggregation =
        prelude::TableAggregation::average("score");
    let _root_expansion: root::TableExpansionState = root::TableExpansionState::all();
    let _prelude_expansion: prelude::TableExpansionState =
        prelude::TableExpansionState::rows([prelude::TableRowId::new("group:team=ui")]);
    let _root_expansion_mode: root::TableExpansionMode = root::TableExpansionMode::Manual;
    let _prelude_expansion_mode: prelude::TableExpansionMode = prelude::TableExpansionMode::Client;
    let _root_stage_mode: root::TableStageMode = root::TableStageMode::Manual;
    let _prelude_stage_mode: prelude::TableStageMode = prelude::TableStageMode::Client;
    let root_filter = root::TableFilter::one_of("status", ["Ready", "Blocked"]);
    let _prelude_filter: prelude::TableFilter = prelude::TableFilter::contains("team", "UI");
    let _root_filter_kind: root::TableFilterKind = root_filter.kind().clone();
    let _prelude_filter_kind: prelude::TableFilterKind =
        prelude::TableFilterKind::Contains { query: "UI".into() };
    let _root_text_filter_operator: root::TableTextFilterOperator =
        root::TableTextFilterOperator::StartsWith;
    let _prelude_text_filter_operator: prelude::TableTextFilterOperator =
        prelude::TableTextFilterOperator::NotContains;
    let _root_numeric_bound: root::TableNumericFilterBound =
        root::TableNumericFilterBound::new(10.0)
            .expect("finite numeric bounds should be constructible");
    let _prelude_numeric_bound: prelude::TableNumericFilterBound =
        prelude::TableNumericFilterBound::new(20.0)
            .expect("finite numeric bounds should be constructible");
    let _root_numeric_filter_operator: root::TableNumericFilterOperator =
        root::TableNumericFilterOperator::GreaterThanOrEqual;
    let _prelude_numeric_filter_operator: prelude::TableNumericFilterOperator =
        prelude::TableNumericFilterOperator::LessThan;
    let root_range_filter = root::TableFilter::number_range("score", Some(10.0), Some(20.0))
        .expect("exported numeric range filter should construct");
    assert_eq!(
        root_range_filter.number_range_bounds(),
        Some((Some(10.0), Some(20.0)))
    );
    let root_facet_value = root::TableFacetValueCount::new("Ready", 2);
    let root_facets: root::TableColumnFacets =
        root::TableColumnFacets::manual("status", 2).with_unique_values([root_facet_value]);
    let _prelude_facets: prelude::TableColumnFacets = root_facets.clone();
    let root_global_facets: root::TableGlobalFacetSummary =
        root::TableGlobalFacetSummary::default();
    let _prelude_global_facets: prelude::TableGlobalFacetSummary = root_global_facets.clone();
    let root_global_filter: root::TableGlobalFilter =
        root::TableGlobalFilter::new("root-global-filter", "Search").query("ready");
    let _root_global_filter_state: root::TableGlobalFilterState = root_global_filter.state();
    let _root_global_filter_change: root::TableGlobalFilterChange =
        root::TableGlobalFilterChange::new("ready");
    let root_predicate_operator: root::TablePredicateFilterOperator =
        root::TablePredicateFilterOperator::text(root::TableTextFilterOperator::StartsWith);
    let root_predicate_filter: root::TablePredicateFilter =
        root::TablePredicateFilter::new("root-name-predicate", "Name", "name")
            .operator(root_predicate_operator)
            .value("Al");
    let root_predicate_filter_state: root::TablePredicateFilterState =
        root_predicate_filter.state();
    let _root_predicate_option: Option<&root::TablePredicateFilterOperatorOptionState> =
        root_predicate_filter_state.operator_options().first();
    let _root_predicate_change: root::TablePredicateFilterChange =
        root::TablePredicateFilterChange::new("name", root_predicate_operator, "Al");
    let root_table_toolbar: root::TableToolbar =
        root::TableToolbar::new("root-table-toolbar", "Filters").summary("2 visible controls");
    let root_table_toolbar_state: root::TableToolbarState = root_table_toolbar.state();
    let _root_table_toolbar_colors: root::TableToolbarColors = root_table_toolbar_state.colors();
    let prelude_global_filter: prelude::TableGlobalFilter =
        prelude::TableGlobalFilter::new("prelude-global-filter", "Search").default_query("ready");
    let _prelude_global_filter_state: prelude::TableGlobalFilterState =
        prelude_global_filter.state();
    let _prelude_global_filter_change: prelude::TableGlobalFilterChange =
        prelude::TableGlobalFilterChange::clear();
    let prelude_predicate_operator: prelude::TablePredicateFilterOperator =
        prelude::TablePredicateFilterOperator::number(
            prelude::TableNumericFilterOperator::GreaterThan,
        );
    let prelude_predicate_filter: prelude::TablePredicateFilter =
        prelude::TablePredicateFilter::new("prelude-score-predicate", "Score", "score")
            .operator(prelude_predicate_operator)
            .default_value("10");
    let prelude_predicate_filter_state: prelude::TablePredicateFilterState =
        prelude_predicate_filter.state();
    let _prelude_predicate_option: Option<&prelude::TablePredicateFilterOperatorOptionState> =
        prelude_predicate_filter_state.operator_options().first();
    let _prelude_predicate_change: prelude::TablePredicateFilterChange =
        prelude::TablePredicateFilterChange::clear("score");
    let prelude_table_toolbar: prelude::TableToolbar =
        prelude::TableToolbar::new("prelude-table-toolbar", "Filters")
            .summary("2 visible controls");
    let prelude_table_toolbar_state: prelude::TableToolbarState = prelude_table_toolbar.state();
    let _prelude_table_toolbar_colors: prelude::TableToolbarColors =
        prelude_table_toolbar_state.colors();
    let root_faceted_filter: root::TableFacetedFilter =
        root::TableFacetedFilter::new("root-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let root_faceted_filter_state: root::TableFacetedFilterState = root_faceted_filter.state();
    let _root_faceted_option: Option<&root::TableFacetedFilterOptionState> =
        root_faceted_filter_state.options().first();
    let _root_faceted_change: root::TableFacetedFilterChange =
        root::TableFacetedFilterChange::new("status", ["Ready"], Some("Ready"), true);
    let prelude_faceted_filter: prelude::TableFacetedFilter =
        prelude::TableFacetedFilter::new("prelude-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let prelude_faceted_filter_state: prelude::TableFacetedFilterState =
        prelude_faceted_filter.state();
    let _prelude_faceted_option: Option<&prelude::TableFacetedFilterOptionState> =
        prelude_faceted_filter_state.options().first();
    let _prelude_faceted_change: prelude::TableFacetedFilterChange =
        prelude::TableFacetedFilterChange::clear("status");
    let root_column_visibility: root::TableColumnVisibility =
        root::TableColumnVisibility::new("root-columns", "Columns")
            .columns([
                root::TableColumn::new("name", "Name").with_hideable(false),
                root::TableColumn::new("status", "Status"),
            ])
            .visibility(root::TableColumnVisibilityOverrides::new().hide("status"));
    let root_column_visibility_state: root::TableColumnVisibilityState =
        root_column_visibility.state();
    let _root_column_visibility_item: Option<&root::TableColumnVisibilityItemState> =
        root_column_visibility_state.items().first();
    let root_column_visibility_change: root::TableColumnVisibilityChange =
        root::TableColumnVisibilityChange::new("status", false);
    let _root_column_visibility_action: root::TableColumnVisibilityAction =
        root_column_visibility_change.action();
    let root_column_order_change: root::TableColumnOrderChange =
        root::TableColumnOrderChange::move_before("score", "team", root::TableColumnRegion::Center);
    let _root_column_order_placement: root::TableColumnOrderPlacement =
        root_column_order_change.placement();
    let prelude_column_visibility: prelude::TableColumnVisibility =
        prelude::TableColumnVisibility::new("prelude-columns", "Columns")
            .columns([prelude::TableColumn::new("status", "Status")])
            .default_visibility(prelude::TableColumnVisibilityOverrides::new().hide("status"));
    let prelude_column_visibility_state: prelude::TableColumnVisibilityState =
        prelude_column_visibility.state();
    let _prelude_column_visibility_item: Option<&prelude::TableColumnVisibilityItemState> =
        prelude_column_visibility_state.items().first();
    let prelude_column_visibility_change: prelude::TableColumnVisibilityChange =
        prelude::TableColumnVisibilityChange::reset();
    let _prelude_column_visibility_action: prelude::TableColumnVisibilityAction =
        prelude_column_visibility_change.action();
    let _prelude_column_order_change: prelude::TableColumnOrderChange = root_column_order_change;
    let _prelude_column_order_placement: prelude::TableColumnOrderPlacement =
        prelude::TableColumnOrderPlacement::After;
    let _root_facet_range: Option<root::TableFacetRange> = root::TableFacetRange::new(1.0, 2.0);
    let root_range_facets =
        root::TableColumnFacets::manual("score", 2).with_numeric_range(1.0, 20.0);
    let root_range_filter: root::TableRangeFilter =
        root::TableRangeFilter::new("root-score-range", "Score", "score")
            .facets(root_range_facets.clone())
            .range(Some(1.0), Some(20.0));
    let _root_range_filter_state: root::TableRangeFilterState = root_range_filter.state();
    let _root_range_change: root::TableRangeFilterChange =
        root::TableRangeFilterChange::new("score", "1", "20");
    let prelude_range_filter: prelude::TableRangeFilter =
        prelude::TableRangeFilter::new("prelude-score-range", "Score", "score")
            .facets(root_range_facets)
            .range(Some(1.0), Some(20.0));
    let _prelude_range_filter_state: prelude::TableRangeFilterState = prelude_range_filter.state();
    let _prelude_range_change: prelude::TableRangeFilterChange =
        prelude::TableRangeFilterChange::clear("score");
    let _prelude_facet_value: prelude::TableFacetValueCount =
        prelude::TableFacetValueCount::new("Blocked", 1);
    let _root_child_load_state: root::TableRowChildrenLoadState =
        root::TableRowChildrenLoadState::loading("Loading children");
    let _prelude_child_load_state: prelude::TableRowChildrenLoadState =
        prelude::TableRowChildrenLoadState::failed("Load failed");
    let _prelude_row_kind: prelude::TableResolvedRowKind = prelude::TableResolvedRowKind::Leaf;
    let root_tree_state = root::TableState::new([root::TableRow::new("root")
        .with_cell("name", "Root")
        .with_child(root::TableRow::new("child").with_cell("name", "Child"))])
    .with_columns([root::TableColumn::new("name", "Name")])
    .with_all_rows_expanded();
    let root_tree_row: root::TableTreeRow = root_tree_state.resolve().final_model().rows()[0]
        .tree()
        .expect("tree source row should expose hierarchy metadata")
        .clone();
    let _prelude_tree_row: prelude::TableTreeRow = root_tree_row;
    let _resolved_kind: Option<&root::TableGroupRow> =
        table.state().resolve().final_model().rows()[0].group();
    let _root_table_modifiers: root::TableInputModifiers = root::TableInputModifiers::default();
    let _prelude_table_modifiers: prelude::TableInputModifiers =
        prelude::TableInputModifiers::default();
    let _root_row_action: Option<root::TableRowAction> = None;
    let _prelude_row_action: Option<prelude::TableRowAction> = None;
    let _root_row_activation: Option<root::TableRowActivation> = None;
    let _prelude_row_activation: Option<prelude::TableRowActivation> = None;
    let _root_row_expansion: Option<root::TableRowExpansionToggle> = None;
    let _prelude_row_expansion: Option<prelude::TableRowExpansionToggle> = None;
    let _root_activation_kind: root::TableRowActivationKind =
        root::TableRowActivationKind::DoubleClick;
    let _prelude_activation_kind: prelude::TableRowActivationKind =
        prelude::TableRowActivationKind::Keyboard;
    let _root_pinning: root::TableColumnPinning =
        root::TableColumnPinning::new().pinned_left(["name"]);
    let _root_width_policy: root::TableColumnWidthPolicy = root::TableColumnWidthPolicy::ContentFit;
    let _prelude_width_policy: prelude::TableColumnWidthPolicy =
        prelude::TableColumnWidthPolicy::Fixed;
    let content_fit_column = root::TableColumn::new("status", "Status").with_content_fit();
    assert!(content_fit_column.is_content_fit());
    assert_eq!(
        content_fit_column.width_policy(),
        root::TableColumnWidthPolicy::ContentFit
    );
    let root_visibility = root::TableColumnVisibilityOverrides::new()
        .hide("score")
        .show("status")
        .without("missing");
    let _root_visibility: root::TableColumnVisibilityOverrides = root_visibility.clone();
    let _prelude_visibility: prelude::TableColumnVisibilityOverrides =
        prelude::TableColumnVisibilityOverrides::new().show("status");
    assert_eq!(
        root_visibility.override_for(&root::TableColumnId::new("score")),
        Some(false)
    );
    let root_sizing = root::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let _root_sizing: root::TableColumnSizing = root_sizing.clone();
    let _prelude_sizing: prelude::TableColumnSizing =
        prelude::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let root_resize_state = root::TableColumnResizeState::begin(
        "name",
        ui_px(12.0),
        ui_px(180.0),
        [("name", ui_px(180.0))],
    );
    let root_resize_update: root::TableColumnResizeUpdate = root::drag_table_column_resize(
        root::TableColumnResizeMode::OnChange,
        root::TableColumnResizeDirection::Ltr,
        &root_sizing,
        &root_resize_state,
        ui_px(24.0),
    );
    let _prelude_resize_state: prelude::TableColumnResizeState = root_resize_update.state().clone();
    let _prelude_resize_update: prelude::TableColumnResizeUpdate = root::end_table_column_resize(
        prelude::TableColumnResizeMode::OnEnd,
        prelude::TableColumnResizeDirection::Ltr,
        &prelude::TableColumnSizing::new().with_width("name", ui_px(180.0)),
        &root_resize_state,
        Some(ui_px(24.0)),
    );
    let root_resize_change = root::TableColumnSizingChange::new(
        "name",
        ui_px(204.0),
        root_resize_update
            .committed_sizing()
            .cloned()
            .expect("resize update should commit in on-change mode"),
    );
    let _prelude_resize_change: prelude::TableColumnSizingChange = root_resize_change;
    let _root_resolved_sizing: root::TableResolvedColumnSizing = table
        .state()
        .resolve()
        .visible_column_sizing()
        .column(&root::TableColumnId::new("name"))
        .expect("resolved column sizing should be available")
        .clone();
    let _prelude_resolved_sizing: prelude::TableResolvedColumnSizing =
        _root_resolved_sizing.clone();
    let _root_resolved_sizing_regions: root::TableResolvedColumnSizingRegions =
        table.state().resolve().visible_column_sizing().clone();
    let _prelude_resolved_sizing_regions: prelude::TableResolvedColumnSizingRegions =
        _root_resolved_sizing_regions.clone();
    let _root_default_width = root::TABLE_DEFAULT_COLUMN_WIDTH;
    let _root_min_width = root::TABLE_MIN_COLUMN_WIDTH;
    let _root_max_width = root::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_default_width = prelude::TABLE_DEFAULT_COLUMN_WIDTH;
    let _prelude_min_width = prelude::TABLE_MIN_COLUMN_WIDTH;
    let _prelude_max_width = prelude::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_region: prelude::TableColumnRegion = prelude::TableColumnRegion::Center;
    let _prelude_regions: prelude::TableColumnRegions =
        table.state().resolve().visible_column_regions().clone();

    assert_eq!(root_snapshot.role(), Role::Table);
    assert!(!root_snapshot.columns().is_empty());
    assert_eq!(
        root::TableRowActivationKind::DoubleClick.as_str(),
        "double-click"
    );
    assert_eq!(virtualizer.resolve().overscan(), 2);
}

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

#[open_gpui::test]
fn table_runtime_resize_emits_controlled_sizing_change(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = sample_table_state(12)
                .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(160.0)));
            let table = Table::new("resize-runtime-table", "Resize runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .column_resize_mode(TableColumnResizeMode::OnEnd)
                .on_column_sizing_change(move |change, _, _| {
                    changes.borrow_mut().push(change);
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let handle = cx
        .debug_bounds("table:resize-runtime-table:resize:name")
        .expect("name resize handle should be rendered")
        .center();

    cx.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(handle.x + px(18.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_move(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_up(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].column_id().as_str(), "name");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(
        changes[0]
            .sizing()
            .width(changes[0].column_id())
            .expect("controlled sizing should include resized column"),
        changes[0].width()
    );
}

#[open_gpui::test]
fn table_runtime_header_drag_emits_controlled_column_order_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnOrderChange>>>,
        state: Rc<RefCell<TableState>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = self.state.borrow().clone();
            let state_for_order = self.state.clone();
            let table = Table::new("order-runtime-table", "Order runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_column_order_change(move |change, _, _| {
                    changes.borrow_mut().push(change.clone());
                    let next = change.apply_to(state_for_order.borrow().clone());
                    *state_for_order.borrow_mut() = next;
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 42_usize)])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ])
        .with_column_order(["name", "team", "score"])
        .with_pagination(TablePagination::disabled()),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
        state: state.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let start = cx
        .debug_bounds("table:order-runtime-table:header:score")
        .expect("score header should render")
        .center();
    let end = cx
        .debug_bounds("table:order-runtime-table:header-order-drop:before:team")
        .expect("team before-drop zone should render")
        .center();

    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.2,
            start.y + (end.y - start.y) * 0.2,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.6,
            start.y + (end.y - start.y) * 0.6,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.target_column_id().as_str(), "team");
    assert_eq!(change.placement(), TableColumnOrderPlacement::Before);
    assert_eq!(change.source_region(), TableColumnRegion::Center);
    assert_eq!(change.target_region(), TableColumnRegion::Center);
    assert_eq!(
        state
            .borrow()
            .column_order()
            .iter()
            .map(|column_id| column_id.as_str())
            .collect::<Vec<_>>(),
        ["name", "score", "team"]
    );
    assert!(
        cx.debug_bounds("table:order-runtime-table:header:score")
            .expect("score header should still render")
            .left()
            < cx.debug_bounds("table:order-runtime-table:header:team")
                .expect("team header should still render")
                .left(),
        "score should render before team after the reorder"
    );
}

#[open_gpui::test]
fn table_runtime_exposes_pinned_region_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
            let table = Table::new("pinned-runtime-table", "Pinned runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(520.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for region in ["left", "center", "right"] {
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:header-region:{region}"
            ))
            .is_some(),
            "expected header {region} region selector to render"
        );
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:row-region:row-a:{region}"
            ))
            .is_some(),
            "expected body {region} region selector to render"
        );
    }

    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:header-center-scroll")
            .is_some(),
        "expected pinned header center region to render a horizontal scroll viewport"
    );
    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:row-center-scroll:row-a")
            .is_some(),
        "expected pinned body center region to render a horizontal scroll viewport"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_center_scrolls_without_moving_fixed_lanes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-scroll-runtime-table",
                "Pinned scroll table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should render before horizontal scrolling");
    let body_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should render before horizontal scrolling");
    let left_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-scroll-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-64.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should remain rendered after horizontal scrolling");
    let body_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should remain rendered after horizontal scrolling");
    let left_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should remain rendered after horizontal scrolling");

    assert!(
        header_center_after.left() < header_center_before.left(),
        "expected shared horizontal handle to move center header left; before={header_center_before:?} after={header_center_after:?}"
    );
    assert!(
        body_center_after.left() < body_center_before.left(),
        "expected horizontal body center lane to move left; before={body_center_before:?} after={body_center_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected right pinned lane to keep its screen-space x position"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_mounts_only_rendered_center_cells(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-runtime-table",
                "Center window runtime table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_some(),
        "expected the first center header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_some(),
        "expected the first center body cell to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_none(),
        "far-right center headers should stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_none(),
        "far-right center body cells should stay unmounted before horizontal scrolling"
    );

    let left_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:center-window-runtime-table:row-center-scroll:row-0000")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_none(),
        "leftmost center headers should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_none(),
        "leftmost center cells should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_some(),
        "far-right center headers should render after horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "far-right center cells should render after horizontal scrolling"
    );

    let left_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should remain rendered after horizontal scrolling");
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "left pinned lane should keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "right pinned lane should keep its screen-space x position"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_still_emits_sort_for_rendered_center_header(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "center-window-sort-runtime-table",
                "Center window sort table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0)
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-sort-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let metric_05_header = cx
        .debug_bounds("table:center-window-sort-runtime-table:header:metric_05")
        .expect("virtualized center header should render after horizontal scrolling");
    cx.simulate_click(metric_05_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].column_id().as_str(), "metric_05");
    assert_eq!(actions[0].label(), "Metric 05");
    assert_eq!(actions[0].current_direction(), None);
    assert_eq!(
        actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
}

#[test]
fn table_behavior_snapshot_updates_center_column_summary_for_resize() {
    let base_snapshot = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(120.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let base_metric = base_snapshot
        .column(&TableColumnId::new("metric_05"))
        .expect("metric_05 should resolve before resize");

    let resized_snapshot = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(180.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let resized_metric = resized_snapshot
        .column(&TableColumnId::new("metric_05"))
        .expect("metric_05 should resolve after resize");

    assert_eq!(
        base_snapshot.column_regions().center_columns(),
        resized_snapshot.column_regions().center_columns()
    );
    assert!(
        resized_snapshot.column_regions().center_width()
            > base_snapshot.column_regions().center_width()
    );
    assert!(
        resized_metric.width() > base_metric.width(),
        "expected the resized center column to widen"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_keeps_row_virtualizer_independent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-rows-runtime-table",
                "Center window rows runtime table",
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

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-rows-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontal center window should reveal far-right cells before vertical scrolling"
    );

    let first_row_pinned_cell = cx
        .debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain reachable before vertical scrolling");
    cx.simulate_event(ScrollWheelEvent {
        position: first_row_pinned_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0000")
            .is_none(),
        "vertical scrolling should still advance the row virtualizer"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0010")
            .is_some(),
        "row 10 should render after vertical scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_05")
            .is_some(),
        "newly rendered rows should consume the current center column window"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_00")
            .is_none(),
        "off-window center cells should remain unmounted on newly rendered rows"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_body_scrolls_without_moving_parent(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-body-scroll-runtime-table",
                "Pinned body scroll table",
                sample_pinned_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(440.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "pinned-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "pinned-table-wrapper".into())
                                    .h(px(140.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let first_row_before = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
        .expect("first pinned body row should render before vertical scrolling");
    let header_before = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:header-row")
        .expect("pinned table header should render before vertical scrolling");
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_none(),
        "row 10 should start outside the initial pinned body window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:pinned-body-scroll-runtime-table:body-scroll")
        .expect("pinned table body viewport should expose a stable scroll selector");
    let first_row_cell = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:cell:row-0000:name")
        .expect("first pinned body row cell should render before vertical scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: first_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    let header_after = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:header-row")
        .expect("pinned table header should still be rendered after vertical scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside pinned Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert_eq!(
        header_after.top(),
        header_before.top(),
        "expected the table header to stay fixed while the body scrolls; before={header_before:?} after={header_after:?}"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
            .is_none(),
        "expected first pinned row to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the pinned table body"
    );
    assert!(
        viewport.size.width > px(0.0) && first_row_before.top() <= parent_bottom_after.bottom(),
        "pinned body viewport should remain measurable during the test"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keeps_bands_fixed_while_center_scrolls(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80).with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-0000"])
                    .pinned_bottom(["row-0079"]),
            );
            let table = Table::new(
                "row-pinning-runtime-table",
                "Row pinning runtime table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(480.0)).h(px(240.0)).child(
                    ScrollArea::new(
                        "row-pinning-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-table-wrapper".into())
                                    .h(px(164.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:top")
            .is_some(),
        "top row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:center")
            .is_some(),
        "center row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:bottom")
            .is_some(),
        "bottom row-pinning band should expose a stable debug selector"
    );
    let top_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should render before scrolling");
    let bottom_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should render before scrolling");
    let parent_bottom_before = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should render before table scrolling");
    let top_name_before = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should render before horizontal scrolling");
    let top_center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:row-center-scroll:row-0000")
        .expect("top pinned row should expose a horizontal center lane");

    cx.simulate_event(ScrollWheelEvent {
        position: top_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_name_after_horizontal = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should stay mounted after horizontal scrolling");
    assert_eq!(
        top_name_after_horizontal.left(),
        top_name_before.left(),
        "left-pinned cells inside pinned rows should not move with the center lane"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontally scrolled pinned rows should reveal far-right center cells"
    );
    let _center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:body-scroll")
        .expect("center body should expose the vertical scroll viewport");
    let center_row_cell = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0001:name")
        .expect("first center row left-pinned cell should render before center scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: center_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should remain mounted after center scrolling");
    let bottom_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should remain mounted after center scrolling");
    let parent_bottom_after = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should remain mounted after center scrolling");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "top pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        bottom_row_after.top(),
        bottom_row_before.top(),
        "bottom pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "vertical wheel input inside row-pinned Table should not move the outer page"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:row:row-0011")
            .is_some(),
        "center rows should advance independently between pinned bands"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0011:metric_05")
            .is_some(),
        "new center rows should consume the current horizontal center window"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keyboard_navigation_scrolls_to_unrendered_center_row(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80)
                .with_row_pinning(TableRowPinning::new().pinned_top(["row-0000"]));
            let table = Table::new(
                "row-pinning-keyboard-table",
                "Row pinning keyboard table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div()
                    .w(px(480.0))
                    .h(px(164.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(table),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_before = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should render before keyboard navigation");
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_none(),
        "far center row should start outside the rendered virtual window"
    );

    cx.simulate_click(top_row_before.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("end");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should remain mounted after keyboard navigation");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "keyboard navigation into the center region should not move the top pinned band"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_some(),
        "End should scroll an unrendered center row into the center virtual window"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_headers_still_sort_after_center_scroll(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "pinned-sort-runtime-table",
                "Pinned sort table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");
    let header_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:header-center-scroll")
        .expect("header center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-160.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:pinned-sort-runtime-table:header:team")
            .is_some(),
        "center header should remain visible after scrolling"
    );
    let score_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:score")
        .expect("left pinned header should remain visible after scrolling");
    let status_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:status")
        .expect("right pinned header should remain visible after scrolling");

    cx.simulate_click(header_center_viewport.center(), Default::default());
    cx.simulate_click(score_header.center(), Default::default());
    cx.simulate_click(status_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].column_id().as_str(), "team");
    assert_eq!(actions[1].column_id().as_str(), "score");
    assert_eq!(actions[2].column_id().as_str(), "status");
}

#[open_gpui::test]
fn table_runtime_pinned_header_drag_emits_controlled_column_order_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnOrderChange>>>,
        state: Rc<RefCell<TableState>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = self.state.borrow().clone();
            let state_for_order = self.state.clone();
            let table = Table::new("pinned-order-runtime-table", "Pinned order table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_column_order_change(move |change, _, _| {
                    changes.borrow_mut().push(change.clone());
                    let next = change.apply_to(state_for_order.borrow().clone());
                    *state_for_order.borrow_mut() = next;
                });

            div()
                .size_full()
                .child(div().w(px(560.0)).h(px(180.0)).child(table))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
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
        .with_column_order(["name", "team", "score", "status"])
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_pagination(TablePagination::disabled()),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
        state: state.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-order-runtime-table:header-center-scroll")
        .expect("center header should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let score_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header:score")
        .expect("center score header should remain visible after scrolling");
    let _team_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header:team")
        .expect("center team header should remain visible after scrolling");
    let drop_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header-order-drop:before:team")
        .expect("team before-drop zone should render in split pinned layout");

    cx.simulate_mouse_down(score_before.center(), MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(score_before.center().x + px(18.0), score_before.center().y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(
            score_before.center().x + px(42.0),
            score_before.center().y + px(2.0),
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(drop_before.center(), MouseButton::Left, Default::default());
    cx.simulate_mouse_up(drop_before.center(), MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.target_column_id().as_str(), "team");
    assert_eq!(change.placement(), TableColumnOrderPlacement::Before);
    assert_eq!(change.source_region(), TableColumnRegion::Center);
    assert_eq!(change.target_region(), TableColumnRegion::Center);
    assert_eq!(
        state
            .borrow()
            .column_order()
            .iter()
            .map(|column_id| column_id.as_str())
            .collect::<Vec<_>>(),
        ["name", "score", "team", "status"]
    );
    assert!(
        cx.debug_bounds("table:pinned-order-runtime-table:header:score")
            .expect("score header should still render")
            .left()
            < cx.debug_bounds("table:pinned-order-runtime-table:header:team")
                .expect("team header should still render")
                .left(),
        "score should render before team after the reorder"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_resize_handles_emit_changes_for_center_and_pinned_columns(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let table = Table::new(
                "pinned-resize-runtime-table",
                "Pinned resize table",
                sample_pinned_table_state()
                    .with_column_sizing(TableColumnSizing::new().with_width("team", ui_px(160.0))),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .column_resize_mode(TableColumnResizeMode::OnEnd)
            .on_column_sizing_change(move |change, _, _| {
                changes.borrow_mut().push(change);
            });

            div()
                .size_full()
                .child(div().w(px(620.0)).h(px(140.0)).child(table))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let team_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:team")
        .expect("center resize handle should remain reachable in split layout");
    let team_handle = point(
        team_handle_bounds.right() - px(1.0),
        team_handle_bounds.center().y,
    );
    let score_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:score")
        .expect("pinned resize handle should remain reachable");
    let score_handle = point(
        score_handle_bounds.right() - px(1.0),
        score_handle_bounds.center().y,
    );

    cx.simulate_mouse_down(team_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(team_handle.x + px(4.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(24.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(changes.borrow().len(), 1);
    assert_eq!(changes.borrow()[0].column_id().as_str(), "team");

    cx.simulate_mouse_down(score_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(score_handle.x + px(4.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(24.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].column_id().as_str(), "team");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(changes[1].column_id().as_str(), "score");
    assert!(changes[1].width() > ui_px(128.0));
}

#[open_gpui::test]
fn table_runtime_virtualized_body_scrolls_without_moving_parent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new("runtime-table", "Runtime table", sample_table_state(80))
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(120.0))
                .overscan(2);

            div().size_full().child(
                div().w(px(360.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-wrapper".into())
                                    .h(px(132.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_some(),
        "expected first table row to render before scrolling"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_none(),
        "expected row 10 to stay outside the initial overscan window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:runtime-table:body-scroll")
        .expect("table body viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_none(),
        "expected row 0 to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the table body"
    );
}

#[open_gpui::test]
fn table_runtime_cache_invalidates_when_table_state_changes(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        descending: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut state = sample_table_state(20);
            if self.descending {
                state = state.with_sorting([TableSort::descending("score")]);
            }

            let table = Table::new("cache-runtime-table", "Cache runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .overscan(0);

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { descending: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_some(),
        "expected unsorted table to render row 0 first"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_none(),
        "expected last row to stay outside the initial unsorted window"
    );

    view.update(cx, |view, cx| {
        view.descending = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_some(),
        "expected cache invalidation to expose the descending first row"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_none(),
        "expected stale unsorted row window to be replaced"
    );
}

#[open_gpui::test]
fn table_runtime_content_fit_widths_follow_visible_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        long_value: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let status_value = if self.long_value {
                "Ready for release rollout"
            } else {
                "Ready"
            };
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("status", status_value)])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(140.0)),
                TableColumn::new("status", "Status").with_content_fit(),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("content-fit-runtime-table", "Content fit runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { long_value: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_before = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should render before content growth");
    let status_cell_before = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should render before content growth");
    assert_eq!(status_header_before.left(), status_cell_before.left());
    assert_eq!(status_header_before.right(), status_cell_before.right());

    view.update(cx, |view, cx| {
        view.long_value = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_after = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should still render after content growth");
    let status_cell_after = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should still render after content growth");
    assert_eq!(status_header_after.left(), status_cell_after.left());
    assert_eq!(status_header_after.right(), status_cell_after.right());
    assert!(
        (status_header_after.right() - status_header_after.left())
            > (status_header_before.right() - status_header_before.left()),
        "expected the content-fit column to widen when a longer visible value appears"
    );
    assert_eq!(
        cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
            .expect("fixed-width name cell should stay rendered")
            .right()
            - cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
                .expect("fixed-width name cell should stay rendered")
                .left(),
        px(140.0)
    );
}

#[open_gpui::test]
fn table_runtime_measured_row_height_reflows_after_paint(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        state: TableState,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "measured-row-runtime-table",
                "Measured row runtime",
                self.state.clone(),
            )
            .row_height(ui_px(24.0))
            .row_measure_mode(TableRowMeasureMode::Measured)
            .viewport_extent(ui_px(120.0));

            div().w(px(260.0)).h(px(180.0)).child(table)
        }
    }

    let state = TableState::new([
        TableRow::new("row-a").with_cell(
            "description",
            "Measured rows should wrap onto multiple lines when the adapter can grow them from rendered content",
        ),
        TableRow::new("row-b").with_cell("description", "Short"),
    ])
    .with_columns([TableColumn::new("description", "Description").with_width(ui_px(72.0))])
    .with_pagination(TablePagination::disabled());

    let (_, cx) = cx.add_window_view(move |_, _| TestView { state });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_a_after = cx
        .debug_bounds("table:measured-row-runtime-table:row:row-a")
        .expect("measured row A should remain rendered after repaint");
    let row_b_after = cx
        .debug_bounds("table:measured-row-runtime-table:row:row-b")
        .expect("measured row B should remain rendered after repaint");
    assert!(
        row_a_after.bottom() - row_a_after.top() > px(24.0),
        "expected the measured first row to grow taller than the fallback row height"
    );
    assert!(
        row_b_after.top() >= row_a_after.bottom() - px(1.0),
        "expected the second row to sit below the expanded first row after the measurement cache is applied; row_a_after.bottom={:?}, row_b_after.top={:?}",
        row_a_after.bottom(),
        row_b_after.top()
    );
}
