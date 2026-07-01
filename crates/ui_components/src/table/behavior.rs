use std::collections::BTreeSet;

use open_gpui_ui_core::{
    Role, TableCellEditor, TableCellValue, TableColumnFacets, TableColumnId, TableColumnRegion,
    TableColumnWidthPolicy, TableGlobalFacetSummary, TableResolvedRow, TableResolvedState,
    TableRowChildrenLoadState, TableRowId, TableRowPinningPolicy, TableRowRegion,
    TableSelectOption, TableSelectionPolicy, TableSelectionSummary, TableSortDirection,
    TableStageMode, TableState, UiPx, VirtualizerRange,
};

use super::render_plan::{TableCellRenderPlan, TableColumnRenderPlan, TableRenderPlan};
use super::{TableHeaderAction, TableMetrics, TableRowMeasureMode};

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
    columns: Vec<TableColumnBehaviorSnapshot>,
    rows: Vec<TableRowBehaviorSnapshot>,
    column_facets: Vec<TableColumnFacets>,
    global_facet_summary: TableGlobalFacetSummary,
    role: Role,
    row_role: Role,
    column_header_role: Role,
    cell_role: Role,
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
            columns,
            rows,
            column_facets: plan.column_facets().to_vec(),
            global_facet_summary: plan.global_facet_summary().clone(),
            role: plan.role(),
            row_role: plan.row_role(),
            column_header_role: plan.column_header_role(),
            cell_role: plan.cell_role(),
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

    /// Returns a currently rendered row by stable row id.
    pub fn row(&self, id: &TableRowId) -> Option<&TableRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.id() == id)
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
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.row_role
    }

    /// Returns the accessibility role for header cells.
    pub const fn column_header_role(&self) -> Role {
        self.column_header_role
    }

    /// Returns the accessibility role for body cells.
    pub const fn cell_role(&self) -> Role {
        self.cell_role
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

/// Row-model and rendered-row counts for a table behavior snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowCountSnapshot {
    core_rows: usize,
    filtered_rows: usize,
    grouped_rows: usize,
    sorted_rows: usize,
    expanded_rows: usize,
    paginated_rows: usize,
    final_rows: usize,
    pinned_top_rows: usize,
    pinned_center_rows: usize,
    pinned_bottom_rows: usize,
    rendered_rows: usize,
    visible_rows: usize,
    aria_rows: usize,
    selected_rows: usize,
    group_rows: usize,
    leaf_rows: usize,
}

impl TableRowCountSnapshot {
    fn from_table(plan: &TableRenderPlan, table: &TableResolvedState, group_rows: usize) -> Self {
        let final_rows = table.final_model().rows().len();
        Self {
            core_rows: table.core_model().rows().len(),
            filtered_rows: table.filtered_model().rows().len(),
            grouped_rows: table.grouped_model().rows().len(),
            sorted_rows: table.sorted_model().rows().len(),
            expanded_rows: table.expanded_model().rows().len(),
            paginated_rows: table.paginated_model().rows().len(),
            final_rows,
            pinned_top_rows: table.top_rows().len(),
            pinned_center_rows: table.center_rows().len(),
            pinned_bottom_rows: table.bottom_rows().len(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            aria_rows: plan.aria_row_count(),
            selected_rows: table.final_model().selected_count(),
            group_rows,
            leaf_rows: final_rows.saturating_sub(group_rows),
        }
    }

    /// Returns the untransformed source row count.
    pub const fn core_rows(self) -> usize {
        self.core_rows
    }

    /// Returns the filtered row count.
    pub const fn filtered_rows(self) -> usize {
        self.filtered_rows
    }

    /// Returns the grouped row-model count.
    pub const fn grouped_rows(self) -> usize {
        self.grouped_rows
    }

    /// Returns the sorted row-model count.
    pub const fn sorted_rows(self) -> usize {
        self.sorted_rows
    }

    /// Returns the expanded row-model count.
    pub const fn expanded_rows(self) -> usize {
        self.expanded_rows
    }

    /// Returns the paginated row-model count.
    pub const fn paginated_rows(self) -> usize {
        self.paginated_rows
    }

    /// Returns the final row-model count.
    pub const fn final_rows(self) -> usize {
        self.final_rows
    }

    /// Returns top-pinned row count.
    pub const fn pinned_top_rows(self) -> usize {
        self.pinned_top_rows
    }

    /// Returns center row count after row pinning.
    pub const fn pinned_center_rows(self) -> usize {
        self.pinned_center_rows
    }

    /// Returns bottom-pinned row count.
    pub const fn pinned_bottom_rows(self) -> usize {
        self.pinned_bottom_rows
    }

    /// Returns the number of body rows rendered after overscan.
    pub const fn rendered_rows(self) -> usize {
        self.rendered_rows
    }

    /// Returns the visible body row count before overscan.
    pub const fn visible_rows(self) -> usize {
        self.visible_rows
    }

    /// Returns the accessibility row count including the header row.
    pub const fn aria_rows(self) -> usize {
        self.aria_rows
    }

    /// Returns selected final row count.
    pub const fn selected_rows(self) -> usize {
        self.selected_rows
    }

    /// Returns synthetic group row count in the final model.
    pub const fn group_rows(self) -> usize {
        self.group_rows
    }

    /// Returns leaf row count in the final model.
    pub const fn leaf_rows(self) -> usize {
        self.leaf_rows
    }
}

/// Visible row window summary without exposing virtualizer internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableVisibleRowsSnapshot {
    visible_range: VirtualizerRange,
    overscan_range: VirtualizerRange,
    rendered_rows: usize,
    visible_rows: usize,
    center_overscan_count: usize,
}

impl TableVisibleRowsSnapshot {
    fn from_render_plan(
        plan: &TableRenderPlan,
        visible_range: &VirtualizerRange,
        overscan_range: &VirtualizerRange,
    ) -> Self {
        Self {
            visible_range: visible_range.clone(),
            overscan_range: overscan_range.clone(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            center_overscan_count: plan.center_overscan_count(),
        }
    }

    /// Returns the visible row range before overscan.
    pub const fn visible_range(&self) -> &VirtualizerRange {
        &self.visible_range
    }

    /// Returns the rendered row range after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        &self.overscan_range
    }

    /// Returns the visible range start index.
    pub const fn visible_start(&self) -> usize {
        self.visible_range.start()
    }

    /// Returns the visible range end index.
    pub const fn visible_end(&self) -> usize {
        self.visible_range.end()
    }

    /// Returns the overscan range start index.
    pub const fn overscan_start(&self) -> usize {
        self.overscan_range.start()
    }

    /// Returns the overscan range end index.
    pub const fn overscan_end(&self) -> usize {
        self.overscan_range.end()
    }

    /// Returns the number of body rows rendered after overscan.
    pub const fn rendered_rows(&self) -> usize {
        self.rendered_rows
    }

    /// Returns the visible body row count before overscan.
    pub const fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    /// Returns the center-row overscan budget used by the vertical virtualizer.
    pub const fn center_overscan_count(&self) -> usize {
        self.center_overscan_count
    }
}

/// Visible column region summary without exposing render-plan columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableColumnRegionSnapshot {
    left_columns: usize,
    center_columns: usize,
    right_columns: usize,
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
    aria_columns: usize,
    resizable_columns: usize,
    row_pinning_policy: TableRowPinningPolicy,
}

impl TableColumnRegionSnapshot {
    fn from_render_plan(plan: &TableRenderPlan, table: &TableResolvedState) -> Self {
        let regions = table.visible_column_regions();
        Self {
            left_columns: regions.left().len(),
            center_columns: regions.center().len(),
            right_columns: regions.right().len(),
            left_width: plan.column_region_width(TableColumnRegion::Left),
            center_width: plan.column_region_width(TableColumnRegion::Center),
            right_width: plan.column_region_width(TableColumnRegion::Right),
            total_width: plan.total_column_width(),
            aria_columns: plan.aria_column_count(),
            resizable_columns: plan
                .columns()
                .iter()
                .filter(|column| column.resizable())
                .count(),
            row_pinning_policy: table.row_pinning_policy(),
        }
    }

    /// Returns visible left-pinned column count.
    pub const fn left_columns(self) -> usize {
        self.left_columns
    }

    /// Returns visible unpinned center column count.
    pub const fn center_columns(self) -> usize {
        self.center_columns
    }

    /// Returns visible right-pinned column count.
    pub const fn right_columns(self) -> usize {
        self.right_columns
    }

    /// Returns rounded left-pinned lane width.
    pub const fn left_width(self) -> UiPx {
        self.left_width
    }

    /// Returns rounded center lane width.
    pub const fn center_width(self) -> UiPx {
        self.center_width
    }

    /// Returns rounded right-pinned lane width.
    pub const fn right_width(self) -> UiPx {
        self.right_width
    }

    /// Returns total visible column width.
    pub const fn total_width(self) -> UiPx {
        self.total_width
    }

    /// Returns the accessibility column count.
    pub const fn aria_columns(self) -> usize {
        self.aria_columns
    }

    /// Returns visible resizable column count.
    pub const fn resizable_columns(self) -> usize {
        self.resizable_columns
    }

    /// Returns pinned row visibility policy.
    pub const fn row_pinning_policy(self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns whether pinned rows are limited to the current page.
    pub const fn row_pinning_page_only(self) -> bool {
        matches!(self.row_pinning_policy, TableRowPinningPolicy::PageOnly)
    }

    /// Returns the width for one column region.
    pub fn width_for(self, region: TableColumnRegion) -> UiPx {
        match region {
            TableColumnRegion::Left => self.left_width,
            TableColumnRegion::Center => self.center_width,
            TableColumnRegion::Right => self.right_width,
        }
    }

    /// Returns whether the table has pinned columns that render in separate lanes.
    pub const fn uses_split_pinned_columns(self) -> bool {
        self.left_columns > 0 || self.right_columns > 0
    }
}

/// Header behavior summary without exposing header render-plan rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableHeaderSummarySnapshot {
    header_rows: usize,
    visible_group_headers: usize,
    sticky_header_band_height: UiPx,
}

impl TableHeaderSummarySnapshot {
    fn from_table(plan: &TableRenderPlan, table: &TableResolvedState) -> Self {
        let visible_group_headers = table
            .header_groups()
            .all()
            .flat_map(|group| group.headers().iter())
            .filter(|cell| cell.is_group())
            .map(|cell| cell.source_id().to_owned())
            .collect::<BTreeSet<_>>()
            .len();

        Self {
            header_rows: plan.header_row_count(),
            visible_group_headers,
            sticky_header_band_height: plan.sticky_header_band_height(),
        }
    }

    /// Returns the maximum visible header row count across regions.
    pub const fn header_rows(self) -> usize {
        self.header_rows
    }

    /// Returns the number of visible group header identities.
    pub const fn visible_group_headers(self) -> usize {
        self.visible_group_headers
    }

    /// Returns the table header band height.
    pub const fn sticky_header_band_height(self) -> UiPx {
        self.sticky_header_band_height
    }
}

/// Source tree and grouped-row behavior summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTreeSummarySnapshot {
    tree_rows: usize,
    tree_branch_rows: usize,
    unloaded_tree_branches: usize,
    loading_tree_rows: usize,
    failed_tree_rows: usize,
    tree_depth: usize,
}

impl TableTreeSummarySnapshot {
    fn from_rows(rows: &[TableResolvedRow]) -> Self {
        Self {
            tree_rows: rows.iter().filter(|row| row.tree().is_some()).count(),
            tree_branch_rows: rows.iter().filter(|row| row.is_tree_branch()).count(),
            unloaded_tree_branches: rows
                .iter()
                .filter(|row| {
                    row.is_tree_branch()
                        && row.loaded_child_count() == 0
                        && row
                            .children_load_state()
                            .is_some_and(|state| *state == TableRowChildrenLoadState::Idle)
                })
                .count(),
            loading_tree_rows: rows
                .iter()
                .filter(|row| {
                    row.children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_loading)
                })
                .count(),
            failed_tree_rows: rows
                .iter()
                .filter(|row| {
                    row.children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_failed)
                })
                .count(),
            tree_depth: rows.iter().map(TableResolvedRow::depth).max().unwrap_or(0),
        }
    }

    /// Returns source tree row count.
    pub const fn tree_rows(self) -> usize {
        self.tree_rows
    }

    /// Returns source tree branch row count.
    pub const fn tree_branch_rows(self) -> usize {
        self.tree_branch_rows
    }

    /// Returns unloaded idle branch count.
    pub const fn unloaded_tree_branches(self) -> usize {
        self.unloaded_tree_branches
    }

    /// Returns loading branch count.
    pub const fn loading_tree_rows(self) -> usize {
        self.loading_tree_rows
    }

    /// Returns failed branch count.
    pub const fn failed_tree_rows(self) -> usize {
        self.failed_tree_rows
    }

    /// Returns maximum source tree depth.
    pub const fn tree_depth(self) -> usize {
        self.tree_depth
    }
}

/// User-observable metadata for one visible table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnBehaviorSnapshot {
    id: TableColumnId,
    label: String,
    region: TableColumnRegion,
    aria_column_index: usize,
    sortable: bool,
    editor: Option<TableCellEditor>,
    select_options: Vec<TableSelectOption>,
    width_policy: TableColumnWidthPolicy,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
    width: UiPx,
    resizable: bool,
}

impl TableColumnBehaviorSnapshot {
    fn from_plan(column: &TableColumnRenderPlan) -> Self {
        Self {
            id: column.id().clone(),
            label: column.label().to_owned(),
            region: column.region(),
            aria_column_index: column.aria_column_index(),
            sortable: column.sortable(),
            editor: column.editor(),
            select_options: column.select_options().to_vec(),
            width_policy: column.width_policy(),
            sort_direction: column.sort_direction(),
            sort_action: column.sort_action().cloned(),
            width: column.width(),
            resizable: column.resizable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved pinning region for this column.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns whether this column is sortable in the contract.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether leaf cells in this column render editors.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns the configured editor for leaf cells in this column.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns fixed select options configured for this column.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns the configured width policy for this column.
    pub const fn width_policy(&self) -> TableColumnWidthPolicy {
        self.width_policy
    }

    /// Returns the resolved sort direction for this column, when present.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the header action emitted when this sortable column is activated.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns the resolved column width.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    /// Returns the label exposed to assistive technology.
    pub fn accessible_label(&self) -> String {
        match self.sort_direction {
            Some(direction) => format!("{}, sorted {}", self.label, direction.as_str()),
            None => self.label.clone(),
        }
    }
}

/// User-observable metadata for one rendered table row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowBehaviorSnapshot {
    id: TableRowId,
    region: TableRowRegion,
    model_index: usize,
    region_index: usize,
    aria_row_index: usize,
    selected: bool,
    depth: usize,
    group: bool,
    leaf: bool,
    tree_branch: bool,
    tree_expanded: Option<bool>,
    loaded_child_count: usize,
    children_load_state: Option<TableRowChildrenLoadState>,
    cells: Vec<TableCellBehaviorSnapshot>,
    role: Role,
}

impl TableRowBehaviorSnapshot {
    fn from_plan(row: &super::render_plan::TableRowRenderPlan) -> Self {
        Self {
            id: row.id().clone(),
            region: row.region(),
            model_index: row.model_index(),
            region_index: row.region_index(),
            aria_row_index: row.aria_row_index(),
            selected: row.selected(),
            depth: row.depth(),
            group: row.row().is_group(),
            leaf: row.row().is_leaf(),
            tree_branch: row.is_tree_branch(),
            tree_expanded: row.tree_expanded(),
            loaded_child_count: row.loaded_child_count(),
            children_load_state: row.children_load_state().cloned(),
            cells: row
                .cells()
                .iter()
                .map(TableCellBehaviorSnapshot::from_plan)
                .collect(),
            role: row.role(),
        }
    }

    /// Returns the stable row id.
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns the row-pinning region.
    pub const fn region(&self) -> TableRowRegion {
        self.region
    }

    /// Returns this row's index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.model_index
    }

    /// Returns this row's index inside its row-pinning region.
    pub const fn region_index(&self) -> usize {
        self.region_index
    }

    /// Returns the 1-based accessibility row index, including the header row.
    pub const fn aria_row_index(&self) -> usize {
        self.aria_row_index
    }

    /// Returns whether the row is selected by stable row id.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns this row's resolved hierarchy depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns whether this is a grouped row.
    pub const fn is_group(&self) -> bool {
        self.group
    }

    /// Returns whether this is a source leaf row.
    pub const fn is_leaf(&self) -> bool {
        self.leaf
    }

    /// Returns whether this row is a source tree branch.
    pub const fn is_tree_branch(&self) -> bool {
        self.tree_branch
    }

    /// Returns the source tree expansion state for branch rows.
    pub const fn tree_expanded(&self) -> Option<bool> {
        self.tree_expanded
    }

    /// Returns the number of directly loaded child rows.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns source-row child loading metadata.
    pub const fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.children_load_state.as_ref()
    }

    /// Returns cells in visible column order.
    pub fn cells(&self) -> &[TableCellBehaviorSnapshot] {
        &self.cells
    }

    /// Returns one cell by column id.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellBehaviorSnapshot> {
        self.cells.iter().find(|cell| cell.column_id() == column)
    }

    /// Returns cells for one column region.
    pub fn cells_for_region(
        &self,
        region: TableColumnRegion,
    ) -> impl Iterator<Item = &TableCellBehaviorSnapshot> {
        self.cells
            .iter()
            .filter(move |cell| cell.region() == region)
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// User-observable metadata for one rendered table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellBehaviorSnapshot {
    column_id: TableColumnId,
    value: Option<TableCellValue>,
    text: String,
    select_options: Vec<TableSelectOption>,
    region: TableColumnRegion,
    aria_column_index: usize,
    role: Role,
    width: UiPx,
    editor: Option<TableCellEditor>,
}

impl TableCellBehaviorSnapshot {
    fn from_plan(cell: &TableCellRenderPlan) -> Self {
        Self {
            column_id: cell.column_id().clone(),
            value: cell.value().cloned(),
            text: cell.text().to_owned(),
            select_options: cell.select_options().to_vec(),
            region: cell.region(),
            aria_column_index: cell.aria_column_index(),
            role: cell.role(),
            width: cell.width(),
            editor: cell.editor(),
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the display text resolved from the core cell value.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the select options configured for this resolved leaf cell.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns the resolved scalar value for this cell, when present.
    pub const fn value(&self) -> Option<&TableCellValue> {
        self.value.as_ref()
    }

    /// Returns the resolved pinning region for this cell.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns the accessibility role for this cell.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the resolved width for this body cell.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns whether this resolved leaf cell should render an editor.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns the editor configured for this resolved leaf cell.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
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
                .filter(|row_id| {
                    table
                        .grouped_model()
                        .row(row_id)
                        .is_some_and(|row| row.is_group())
                })
                .count(),
            rows.iter()
                .filter(|row_id| {
                    table
                        .core_model()
                        .row(row_id)
                        .is_some_and(|row| row.is_tree_branch())
                })
                .count(),
        ),
    }
}
