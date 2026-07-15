mod support;

use open_gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement, Styled, Window, div, point, px,
};
use open_gpui_ui_components::{
    PopoverOpenMode, ScrollArea, Table, TableCellEditApplyOutcome, TableCellEditRequest,
    TableColumnOrderChange, TableColumnOrderPlacement, TableColumnSizingChange,
    TableColumnVisibility, TableColumnVisibilityAction, TableColumnVisibilityChange,
    TableColumnVisibilityState, TableFacetedFilter, TableFacetedFilterChange,
    TableFacetedFilterState, TableGlobalFilter, TableGlobalFilterChange, TableGlobalFilterState,
    TableHeaderAction, TablePredicateFilter, TablePredicateFilterChange,
    TablePredicateFilterOperator, TablePredicateFilterOperatorOptionState,
    TablePredicateFilterState, TableRangeFilter, TableRangeFilterChange, TableRangeFilterState,
    TableRowMeasureMode, TableSelectionScope, TableToolbar, TableToolbarState,
    TableVirtualizerSnapshot, TableVirtualizerSnapshotItem,
    gpui_adapter::{TableDebugSelector, init_text_input},
};
use open_gpui_ui_core::{
    OutsidePressPolicy, OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size,
    TableCellEditor, TableCellValue, TableColumn, TableColumnFacets, TableColumnGroup,
    TableColumnId, TableColumnPinning, TableColumnRegion, TableColumnResizeMode, TableColumnSizing,
    TableColumnVisibilityOverrides, TableExpansionMode, TableFacetValueCount, TableFilter,
    TableGlobalFacetSummary, TableNumericFilterOperator, TablePagination, TableRow,
    TableRowChildrenLoadState, TableRowId, TableRowIdentity, TableRowPinning,
    TableRowPinningPolicy, TableRowRegion, TableSelectOption, TableSelectionActivationMode,
    TableSelectionMode, TableSort, TableSortDirection, TableSourceRowIdentity, TableStageMode,
    TableState, TableTextFilterOperator, UiPx, VirtualizerRange, ui_px,
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

fn table_source_row_identity(row_id: impl Into<TableRowId>) -> TableRowIdentity {
    TableRowIdentity::source(row_id)
}

fn table_source_row_selector(table_id: &str, row_id: impl Into<TableRowId>) -> String {
    TableDebugSelector::row(table_id, &table_source_row_identity(row_id))
}

fn table_source_cell_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
) -> String {
    TableDebugSelector::cell(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
    )
}

fn table_source_tree_toggle_selector(table_id: &str, row_id: impl Into<TableRowId>) -> String {
    TableDebugSelector::tree_toggle(table_id, &table_source_row_identity(row_id))
}

fn table_source_row_region_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    region: TableColumnRegion,
) -> String {
    TableDebugSelector::row_region(table_id, &table_source_row_identity(row_id), region)
}

fn table_source_row_center_scroll_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
) -> String {
    TableDebugSelector::row_center_scroll(table_id, &table_source_row_identity(row_id))
}

fn table_source_text_input_editor_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
) -> String {
    TableDebugSelector::text_input_editor_root(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
    )
}

fn table_source_textarea_editor_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
) -> String {
    TableDebugSelector::textarea_editor_root(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
    )
}

fn table_source_checkbox_editor_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
) -> String {
    TableDebugSelector::checkbox_editor_root(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
    )
}

fn table_source_select_editor_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
) -> String {
    TableDebugSelector::select_editor_trigger(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
    )
}

fn table_source_select_option_selector(
    table_id: &str,
    row_id: impl Into<TableRowId>,
    column_id: impl Into<TableColumnId>,
    option_value: &str,
) -> String {
    TableDebugSelector::select_editor_option(
        table_id,
        &table_source_row_identity(row_id),
        &column_id.into(),
        option_value,
    )
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

#[path = "table/accessibility.rs"]
mod accessibility;
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
#[path = "table/runtime_editing.rs"]
mod runtime_editing;
#[path = "table/runtime_interactions.rs"]
mod runtime_interactions;
#[path = "table/runtime_layout.rs"]
mod runtime_layout;
