mod columns;
mod counts;
mod header;
mod rows;
mod tree;

use open_gpui_ui_core::{
    Role, TableColumnFacets, TableColumnId, TableColumnRegion, TableGlobalFacetSummary,
    TableResolvedState, TableRowId, TableRowIdentity, TableRowIdentityDiagnostic, TableRowRegion,
    TableSelectionPolicy, TableSelectionSummary, TableStageMode, TableState, UiPx,
};

use super::render_plan::TableRenderPlan;
use super::{TableMetrics, TableRowMeasureMode};

pub use columns::{TableColumnBehaviorSnapshot, TableColumnRegionSnapshot};
pub use counts::{TableRowCountSnapshot, TableVisibleRowsSnapshot};
pub use header::TableHeaderSummarySnapshot;
pub use rows::{TableCellBehaviorSnapshot, TableRowBehaviorSnapshot};
pub use tree::TableTreeSummarySnapshot;

/// User-observable table behavior resolved for a viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct TableBehaviorSnapshot {
    table_id: String,
    label: String,
    metrics: TableMetrics,
    row_measure_mode: TableRowMeasureMode,
    filtering_mode: TableStageMode,
    sorting_mode: TableStageMode,
    pagination_mode: TableStageMode,
    pagination_page_index: usize,
    pagination_page_size: usize,
    pagination_row_count: Option<usize>,
    pagination_page_count: Option<usize>,
    faceting_mode: TableStageMode,
    selection_policy: TableSelectionPolicy,
    selection_summary: TableSelectionSummary,
    aggregation_count: usize,
    aggregation_fn_count: usize,
    grouping_columns: Vec<TableColumnId>,
    expansion_mode_manual: bool,
    expansion_all: bool,
    expanded_group_inputs: usize,
    expanded_tree_inputs: usize,
    row_counts: TableRowCountSnapshot,
    visible_rows: TableVisibleRowsSnapshot,
    column_regions: TableColumnRegionSnapshot,
    header_summary: TableHeaderSummarySnapshot,
    tree_summary: TableTreeSummarySnapshot,
    row_identity_diagnostics: Vec<TableRowIdentityDiagnostic>,
    columns: Vec<TableColumnBehaviorSnapshot>,
    rows: Vec<TableRowBehaviorSnapshot>,
    column_facets: Vec<TableColumnFacets>,
    global_facet_summary: TableGlobalFacetSummary,
}

impl TableBehaviorSnapshot {
    pub(in crate::table) fn from_render_plan(plan: &TableRenderPlan, state: &TableState) -> Self {
        let table = plan.table();
        let visible = plan.virtualizer().visible_range();
        let overscan = plan.virtualizer().overscan_range();
        let final_rows = table.final_model().rows();
        let group_rows = final_rows.iter().filter(|row| row.is_group()).count();
        let tree_summary = TableTreeSummarySnapshot::from_rows(final_rows);
        let row_counts = TableRowCountSnapshot::from_table(plan, table, group_rows);
        let visible_rows = TableVisibleRowsSnapshot::from_render_plan(plan, visible, overscan);
        let column_regions = TableColumnRegionSnapshot::from_render_plan(plan, table);
        let header_summary = TableHeaderSummarySnapshot::from_table(plan, table);
        let columns = plan
            .columns()
            .iter()
            .map(TableColumnBehaviorSnapshot::from_plan)
            .collect();
        let rows = plan
            .rendered_rows()
            .map(TableRowBehaviorSnapshot::from_plan)
            .collect();
        let (expansion_all, expanded_group_inputs, expanded_tree_inputs) =
            resolved_expansion_inputs(state, table);

        Self {
            table_id: plan.table_id().to_owned(),
            label: plan.label().to_owned(),
            metrics: plan.metrics(),
            row_measure_mode: plan.row_measure_mode(),
            filtering_mode: plan.filtering_mode(),
            sorting_mode: plan.sorting_mode(),
            pagination_mode: plan.pagination_mode(),
            pagination_page_index: state.pagination().page_index(),
            pagination_page_size: state.pagination().page_size(),
            pagination_row_count: plan.pagination_row_count(),
            pagination_page_count: plan.pagination_page_count(),
            faceting_mode: plan.faceting_mode(),
            selection_policy: plan.selection_policy(),
            selection_summary: plan.selection_summary(),
            aggregation_count: state.aggregations().len(),
            aggregation_fn_count: plan.aggregation_fn_count(),
            grouping_columns: state.grouping().to_vec(),
            expansion_mode_manual: matches!(
                state.expansion_mode(),
                open_gpui_ui_core::TableExpansionMode::Manual
            ),
            expansion_all,
            expanded_group_inputs,
            expanded_tree_inputs,
            row_counts,
            visible_rows,
            column_regions,
            header_summary,
            tree_summary,
            row_identity_diagnostics: table.row_identity_diagnostics().to_vec(),
            columns,
            rows,
            column_facets: plan.column_facets().to_vec(),
            global_facet_summary: plan.global_facet_summary().clone(),
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

    /// Returns resolved viewport metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns the row height ownership mode.
    pub const fn row_measure_mode(&self) -> TableRowMeasureMode {
        self.row_measure_mode
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

    /// Returns the zero-based page index.
    pub const fn pagination_page_index(&self) -> usize {
        self.pagination_page_index
    }

    /// Returns the configured page size.
    pub const fn pagination_page_size(&self) -> usize {
        self.pagination_page_size
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

    /// Returns the number of configured aggregations.
    pub const fn aggregation_count(&self) -> usize {
        self.aggregation_count
    }

    /// Returns the number of named custom aggregation callbacks registered on the table state.
    pub const fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fn_count
    }

    /// Returns configured grouping columns in outer-to-inner order.
    pub fn grouping_columns(&self) -> &[TableColumnId] {
        &self.grouping_columns
    }

    /// Returns whether expansion is caller-owned.
    pub const fn manual_expansion(&self) -> bool {
        self.expansion_mode_manual
    }

    /// Returns whether every group or tree row is expanded.
    pub const fn all_rows_expanded(&self) -> bool {
        self.expansion_all
    }

    /// Returns explicit expanded group ids, or all group rows when expansion is global.
    pub const fn expanded_group_inputs(&self) -> usize {
        self.expanded_group_inputs
    }

    /// Returns explicit expanded tree row ids, or all tree branches when expansion is global.
    pub const fn expanded_tree_inputs(&self) -> usize {
        self.expanded_tree_inputs
    }

    /// Returns row-model and rendered-row counts.
    pub const fn row_counts(&self) -> TableRowCountSnapshot {
        self.row_counts
    }

    /// Returns visible row window behavior.
    pub const fn visible_rows(&self) -> &TableVisibleRowsSnapshot {
        &self.visible_rows
    }

    /// Returns visible column region behavior.
    pub const fn column_regions(&self) -> TableColumnRegionSnapshot {
        self.column_regions
    }

    /// Returns the summed resolved width of one visible column region.
    pub fn column_region_width(&self, region: TableColumnRegion) -> UiPx {
        self.column_regions.width_for(region)
    }

    /// Returns whether the table behavior requires separated pinned column lanes.
    pub const fn uses_split_pinned_columns(&self) -> bool {
        self.column_regions.uses_split_pinned_columns()
    }

    /// Returns header behavior summary.
    pub const fn header_summary(&self) -> TableHeaderSummarySnapshot {
        self.header_summary
    }

    /// Returns source tree and grouped row behavior summary.
    pub const fn tree_summary(&self) -> TableTreeSummarySnapshot {
        self.tree_summary
    }

    /// Returns structured source-row identity diagnostics detected during resolution.
    pub fn row_identity_diagnostics(&self) -> &[TableRowIdentityDiagnostic] {
        &self.row_identity_diagnostics
    }

    /// Returns visible columns in behavior order.
    pub fn columns(&self) -> &[TableColumnBehaviorSnapshot] {
        &self.columns
    }

    /// Returns a visible column by id.
    pub fn column(&self, id: &TableColumnId) -> Option<&TableColumnBehaviorSnapshot> {
        self.columns.iter().find(|column| column.id() == id)
    }

    /// Returns currently rendered rows in visual order.
    pub fn rows(&self) -> &[TableRowBehaviorSnapshot] {
        &self.rows
    }

    /// Returns a currently rendered row by authoritative resolved identity.
    pub fn row(&self, identity: &TableRowIdentity) -> Option<&TableRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.identity() == identity)
    }

    /// Returns rendered source rows matching a caller-owned business id.
    pub fn source_rows<'a>(
        &'a self,
        row_id: &TableRowId,
    ) -> impl Iterator<Item = &'a TableRowBehaviorSnapshot> + 'a {
        let row_id = row_id.clone();
        self.rows
            .iter()
            .filter(move |row| row.source_row_id() == Some(&row_id))
    }

    /// Returns a rendered source row only when its business id is unique.
    pub fn unique_source_row(&self, row_id: &TableRowId) -> Option<&TableRowBehaviorSnapshot> {
        let mut rows = self.source_rows(row_id);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }

    /// Returns currently rendered rows from one pinning region.
    pub fn rows_for_region(
        &self,
        region: TableRowRegion,
    ) -> impl Iterator<Item = &TableRowBehaviorSnapshot> {
        self.rows.iter().filter(move |row| row.region() == region)
    }

    /// Returns resolved facet metadata for configured columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        &self.column_facets
    }

    /// Returns resolved facet metadata for one configured column.
    pub fn column_facet(&self, column: &TableColumnId) -> Option<&TableColumnFacets> {
        self.column_facets
            .iter()
            .find(|facet| facet.column() == column)
    }

    /// Returns resolved facet metadata for the global filter context.
    pub const fn global_facet_summary(&self) -> &TableGlobalFacetSummary {
        &self.global_facet_summary
    }

    /// Returns the accessibility role for the table root.
    pub const fn role(&self) -> Role {
        Role::Table
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        Role::Row
    }

    /// Returns the accessibility role for header cells.
    pub const fn column_header_role(&self) -> Role {
        Role::ColumnHeader
    }

    /// Returns the accessibility role for body cells.
    pub const fn cell_role(&self) -> Role {
        Role::Cell
    }

    /// Returns the accessibility row count, including the header row.
    pub const fn aria_row_count(&self) -> usize {
        self.row_counts.aria_rows()
    }

    /// Returns the accessibility column count.
    pub const fn aria_column_count(&self) -> usize {
        self.column_regions.aria_columns()
    }

    /// Returns the number of body rows rendered after overscan.
    pub const fn rendered_row_count(&self) -> usize {
        self.row_counts.rendered_rows()
    }

    /// Returns the visible body row count before overscan.
    pub const fn visible_row_count(&self) -> usize {
        self.row_counts.visible_rows()
    }
}

fn resolved_expansion_inputs(
    state: &TableState,
    table: &TableResolvedState,
) -> (bool, usize, usize) {
    match state.expansion() {
        open_gpui_ui_core::TableExpansionState::All => (
            true,
            table
                .grouped_model()
                .rows()
                .iter()
                .filter(|row| row.is_group())
                .count(),
            table
                .core_model()
                .rows()
                .iter()
                .filter(|row| row.is_tree_branch())
                .count(),
        ),
        open_gpui_ui_core::TableExpansionState::Rows(rows) => (
            false,
            rows.iter()
                .filter(|identity| {
                    table
                        .grouped_model()
                        .row(identity)
                        .is_some_and(|row| row.is_group())
                })
                .count(),
            rows.iter()
                .filter(|identity| {
                    table
                        .core_model()
                        .row(identity)
                        .is_some_and(|row| row.is_tree_branch())
                })
                .count(),
        ),
    }
}
