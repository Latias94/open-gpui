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

#[path = "table/behavior_rows.rs"]
mod behavior_rows;
#[path = "table/editing_contracts.rs"]
mod editing_contracts;
#[path = "table/exports.rs"]
mod exports;
#[path = "table/filters_toolbar.rs"]
mod filters_toolbar;
#[path = "table/layout_contracts.rs"]
mod layout_contracts;
#[path = "table/runtime_interactions.rs"]
mod runtime_interactions;
#[path = "table/runtime_layout.rs"]
mod runtime_layout;
