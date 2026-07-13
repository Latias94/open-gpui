use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui_ui_core::{
    RowWindow, TableColumnFacets, TableColumnId, TableColumnRegion, TableGlobalFacetSummary,
    TableResolvedRow, TableResolvedState, TableRowId, TableRowRegion, TableSelectionPolicy,
    TableSelectionSummary, TableStageMode, TableState, UiPx, VirtualizerItemKey,
    VirtualizerItemMeasurement, VirtualizerResolvedState,
};

use crate::table::layout::resolve_column_region_render_plans;

use super::virtualization::row_render_key;
use super::{TableMetrics, TableRowMeasureMode, apply_table_content_fit_widths};

mod columns;
mod header;
mod rows;

pub(in crate::table) use columns::{
    TableCenterColumnWindowPlan, TableColumnRegionRenderPlan, TableColumnRenderPlan,
    TablePinnedLayoutPlan,
};
pub(in crate::table) use header::{TableHeaderCellRenderPlan, TableHeaderGroupRegionsRenderPlan};
pub(in crate::table) use rows::{TableCellRenderPlan, TableRowRenderPlan};

/// Internal adapter render plan for a concrete [`Table`] viewport.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::table) struct TableRenderPlan {
    table_id: String,
    label: String,
    metrics: TableMetrics,
    row_measure_mode: TableRowMeasureMode,
    table: Rc<TableResolvedState>,
    virtualizer: VirtualizerResolvedState,
    content_fit_widths: BTreeMap<TableColumnId, UiPx>,
    columns: Vec<TableColumnRenderPlan>,
    column_regions: Vec<TableColumnRegionRenderPlan>,
    header_groups: TableHeaderGroupRegionsRenderPlan,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_column_window: Option<TableCenterColumnWindowPlan>,
    total_column_width: UiPx,
    filtering_mode: TableStageMode,
    sorting_mode: TableStageMode,
    pagination_mode: TableStageMode,
    pagination_row_count: Option<usize>,
    pagination_page_count: Option<usize>,
    faceting_mode: TableStageMode,
    selection_policy: TableSelectionPolicy,
    selection_summary: TableSelectionSummary,
    aggregation_fn_count: usize,
    top_rows: Vec<TableRowRenderPlan>,
    rows: Vec<TableRowRenderPlan>,
    bottom_rows: Vec<TableRowRenderPlan>,
    center_visible_row_count: usize,
    center_overscan_count: usize,
}

impl TableRenderPlan {
    pub(super) fn resolve(
        table_id: String,
        label: String,
        metrics: TableMetrics,
        row_measure_mode: TableRowMeasureMode,
        state: &TableState,
        table: Rc<TableResolvedState>,
        virtualizer: VirtualizerResolvedState,
        columns: Vec<TableColumnRenderPlan>,
        content_fit_widths: BTreeMap<TableColumnId, UiPx>,
        center_scroll_offset: Option<UiPx>,
        center_viewport_extent: Option<UiPx>,
        row_measurements: &BTreeMap<String, UiPx>,
    ) -> Self {
        let columns =
            apply_table_content_fit_widths(columns, &content_fit_widths, state.column_sizing());
        let column_regions = resolve_column_region_render_plans(&columns);
        let header_groups = TableHeaderGroupRegionsRenderPlan::from_resolved(
            &table_id,
            table.header_groups(),
            &columns,
            &column_regions,
        );
        let header_row_count = header_groups.row_count().max(1);
        let total_column_width = column_regions
            .iter()
            .fold(UiPx::ZERO, |total, region| total + region.total_width());
        let pinned_layout = TablePinnedLayoutPlan::from_column_regions(
            &table_id,
            &column_regions,
            total_column_width,
        );
        let center = column_regions
            .iter()
            .find(|plan| plan.region() == TableColumnRegion::Center);
        let center_column_window = center.and_then(|center| {
            let viewport_extent = center_viewport_extent.unwrap_or_else(|| center.total_width());
            TableCenterColumnWindowPlan::resolve(
                center.columns(),
                center_scroll_offset.unwrap_or(UiPx::ZERO),
                viewport_extent,
                metrics.overscan(),
            )
        });
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let top_row_count = table.top_rows().len();
        let center_total_row_count = table.center_rows().len();
        let top_rows = row_render_plans(
            table.top_rows(),
            TableRowRegion::Top,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            0,
            UiPx::ZERO,
            header_row_count,
        );
        let center_window = virtualized_center_row_window(
            table.center_rows(),
            &columns,
            &virtualizer,
            top_row_count,
            header_row_count,
        );
        let top_height = top_rows
            .iter()
            .fold(UiPx::ZERO, |total, row| total + row.virtual_size());
        let bottom_rows = row_render_plans(
            table.bottom_rows(),
            TableRowRegion::Bottom,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            top_row_count + center_total_row_count,
            top_height + virtualizer.total_size(),
            header_row_count,
        );
        let pagination = state.pagination();
        let selection_summary = table.final_selection_summary();

        Self {
            table_id,
            label,
            metrics,
            row_measure_mode,
            table,
            virtualizer,
            content_fit_widths,
            columns,
            column_regions,
            header_groups,
            pinned_layout,
            center_column_window,
            total_column_width,
            filtering_mode: state.filtering_mode(),
            sorting_mode: state.sorting_mode(),
            pagination_mode: pagination.mode(),
            pagination_row_count: pagination.row_count(),
            pagination_page_count: pagination.page_count(),
            faceting_mode: state.faceting_mode(),
            selection_policy: state.selection_policy(),
            selection_summary,
            aggregation_fn_count: state.aggregation_fn_count(),
            top_rows,
            rows: center_window.rows,
            bottom_rows,
            center_visible_row_count: center_window.visible_row_count,
            center_overscan_count: center_window.overscan_count,
        }
    }

    /// Returns the stable table id.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the accessible table label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns the row height ownership mode.
    pub const fn row_measure_mode(&self) -> TableRowMeasureMode {
        self.row_measure_mode
    }

    /// Returns the resolved renderer-neutral table state.
    pub fn table(&self) -> &TableResolvedState {
        self.table.as_ref()
    }

    /// Returns whether filtering was resolved locally or supplied by the caller.
    pub const fn filtering_mode(&self) -> TableStageMode {
        self.filtering_mode
    }

    /// Returns whether sorting was resolved locally or supplied by the caller.
    pub const fn sorting_mode(&self) -> TableStageMode {
        self.sorting_mode
    }

    /// Returns whether pagination was resolved locally or supplied by the caller.
    pub const fn pagination_mode(&self) -> TableStageMode {
        self.pagination_mode
    }

    /// Returns the server-known total row count, when supplied.
    pub const fn pagination_row_count(&self) -> Option<usize> {
        self.pagination_row_count
    }

    /// Returns the explicit or derived total page count, when supplied.
    pub const fn pagination_page_count(&self) -> Option<usize> {
        self.pagination_page_count
    }

    /// Returns whether faceting was resolved locally or supplied by the caller.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns the row-selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns the final row-model selection summary.
    pub const fn selection_summary(&self) -> TableSelectionSummary {
        self.selection_summary
    }

    /// Returns the number of named custom aggregation callbacks registered on the table state.
    pub const fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fn_count
    }

    /// Returns resolved facet metadata for configured columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        self.table.column_facets()
    }

    /// Returns resolved facet metadata for the global filter context.
    pub fn global_facet_summary(&self) -> &TableGlobalFacetSummary {
        self.table.global_facet_summary()
    }

    /// Returns the resolved renderer-neutral virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns visible columns in render order.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns visible columns split into render regions.
    pub fn column_regions(&self) -> &[TableColumnRegionRenderPlan] {
        &self.column_regions
    }

    /// Returns nested header groups split into render regions.
    pub fn header_groups(&self) -> &TableHeaderGroupRegionsRenderPlan {
        &self.header_groups
    }

    /// Returns the maximum header row count across all regions.
    pub fn header_row_count(&self) -> usize {
        self.header_groups.row_count().max(1)
    }

    /// Returns the total height reserved for the table header band.
    pub fn sticky_header_band_height(&self) -> UiPx {
        self.metrics.header_height() * self.header_row_count() as f32
    }

    /// Returns sticky pinned-column layout metadata, when a split layout is needed.
    pub fn pinned_layout(&self) -> Option<&TablePinnedLayoutPlan> {
        self.pinned_layout.as_ref()
    }

    /// Returns center-column window metadata, when the center lane exists.
    pub fn center_column_window(&self) -> Option<&TableCenterColumnWindowPlan> {
        self.center_column_window.as_ref()
    }

    /// Returns the summed resolved width of all visible columns.
    pub const fn total_column_width(&self) -> UiPx {
        self.total_column_width
    }

    /// Returns the summed resolved width of one visible column region.
    pub fn column_region_width(&self, region: TableColumnRegion) -> UiPx {
        self.column_regions
            .iter()
            .find(|plan| plan.region() == region)
            .map(TableColumnRegionRenderPlan::total_width)
            .unwrap_or(UiPx::ZERO)
    }

    /// Returns top-pinned rows in render order.
    pub fn top_rows(&self) -> &[TableRowRenderPlan] {
        &self.top_rows
    }

    /// Returns virtualized center rows in render order.
    pub fn rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns bottom-pinned rows in render order.
    pub fn bottom_rows(&self) -> &[TableRowRenderPlan] {
        &self.bottom_rows
    }

    /// Returns all currently rendered rows in visual order.
    pub fn rendered_rows(&self) -> impl Iterator<Item = &TableRowRenderPlan> {
        self.top_rows
            .iter()
            .chain(self.rows.iter())
            .chain(self.bottom_rows.iter())
    }

    /// Returns the accessibility row count, including the header row.
    pub fn aria_row_count(&self) -> usize {
        self.table
            .final_model()
            .rows()
            .len()
            .saturating_add(self.header_row_count())
    }

    /// Returns the accessibility column count.
    pub fn aria_column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the number of body rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.top_rows.len() + self.rows.len() + self.bottom_rows.len()
    }

    /// Returns the visible body row count before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.top_rows.len() + self.center_visible_row_count + self.bottom_rows.len()
    }

    /// Returns the center-row overscan budget used by the vertical virtualizer.
    pub const fn center_overscan_count(&self) -> usize {
        self.center_overscan_count
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableCenterRowWindow {
    rows: Vec<TableRowRenderPlan>,
    visible_row_count: usize,
    overscan_count: usize,
}

fn virtualized_center_row_window(
    rows: &[TableResolvedRow],
    columns: &[TableColumnRenderPlan],
    virtualizer: &VirtualizerResolvedState,
    model_index_start: usize,
    header_row_count: usize,
) -> TableCenterRowWindow {
    let row_window = RowWindow::project(virtualizer, |index| rows.get(index).cloned());
    let visible_row_count = row_window.visible_row_count();
    let overscan_count = row_window.overscan_count();
    let rows = row_window
        .into_rows()
        .into_iter()
        .map(|projected| {
            let (index, render_key, measurement, row) = projected.into_parts();
            let model_index = model_index_start + index;
            TableRowRenderPlan::new(
                row,
                TableRowRegion::Center,
                render_key,
                model_index,
                header_row_count,
                measurement,
                columns,
            )
        })
        .collect();

    TableCenterRowWindow {
        rows,
        visible_row_count,
        overscan_count,
    }
}

fn row_render_plans(
    rows: &[TableResolvedRow],
    region: TableRowRegion,
    row_measure_mode: TableRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    fallback_row_height: UiPx,
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
    start_offset: UiPx,
    header_row_count: usize,
) -> Vec<TableRowRenderPlan> {
    let mut cursor = start_offset;
    rows.iter()
        .enumerate()
        .map(|(region_index, row)| {
            let row = row.clone();
            let render_key = row_render_key(&row, duplicate_row_ids);
            let model_index = model_index_start + region_index;
            let row_height = if row_measure_mode.measured() {
                row_measurements
                    .get(&render_key)
                    .copied()
                    .unwrap_or(fallback_row_height)
            } else {
                fallback_row_height
            };
            let measured =
                row_measure_mode.measured() && row_measurements.contains_key(&render_key);
            let measurement = VirtualizerItemMeasurement::new(
                region_index,
                VirtualizerItemKey::new(render_key.clone()),
                cursor,
                row_height,
                measured,
            );
            cursor = measurement.end();
            TableRowRenderPlan::new(
                row,
                region,
                render_key,
                model_index,
                header_row_count,
                measurement,
                columns,
            )
        })
        .collect()
}
