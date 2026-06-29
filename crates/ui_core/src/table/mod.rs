//! Renderer-neutral table row-model contracts for Open GPUI components.

mod aggregation;
mod columns;
mod faceting;
mod filtering;
mod headers;
mod identity;
mod resolved;
mod row_model;
mod rows;
mod selection;
mod sizing;

pub use aggregation::{TableAggregateKind, TableAggregation};
pub use columns::{
    TABLE_DEFAULT_COLUMN_WIDTH, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH, TableColumn,
    TableColumnGroup, TableColumnNode, TableColumnPinning, TableColumnRegion, TableColumnRegions,
    TableColumnVisibilityOverrides, TableColumnWidthPolicy,
};
pub use faceting::{
    TableColumnFacets, TableFacetRange, TableFacetValueCount, TableGlobalFacetSummary,
};
pub use filtering::{
    TableFilter, TableFilterKind, TableNumericFilterBound, TableNumericFilterOperator, TableSort,
    TableSortDirection, TableTextFilterOperator,
};
pub use headers::{
    TableResolvedHeaderCell, TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions,
    TableResolvedHeaderKind,
};
pub use identity::{TableColumnGroupId, TableColumnId, TableRowId};
pub use resolved::{
    TableGroupRow, TableResolvedRow, TableResolvedRowKind, TableResolvedState, TableRowModel,
    TableTreeRow,
};
pub use row_model::{
    TABLE_ROW_MODEL_PIPELINE, TABLE_ROW_MODEL_V0_PIPELINE, TableExpansionMode, TableExpansionState,
    TablePagination, TableRowModelStage, TableStageMode,
};
pub use rows::{
    TableRow, TableRowChildrenLoadState, TableRowPinning, TableRowPinningPolicy, TableRowRegion,
    TableRowRegions,
};
pub use selection::{
    TableSelectionActivationMode, TableSelectionMode, TableSelectionPolicy, TableSelectionSummary,
    TableSelectionSummaryState, TableSubRowSelectionPolicy,
};
pub use sizing::{
    TableColumnResizeDirection, TableColumnResizeMode, TableColumnResizeState,
    TableColumnResizeUpdate, TableColumnSizing, TableResolvedColumnSizing,
    TableResolvedColumnSizingRegions, drag_table_column_resize, end_table_column_resize,
};

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[cfg(test)]
use crate::geometry::ui_px;
use aggregation::TableAggregationFn;
#[cfg(test)]
use aggregation::numeric_values;
use columns::normalize_table_column_tree;
use faceting::{resolve_client_column_facets, resolve_client_global_column_facets};
use filtering::normalize_table_global_filter_query;
use row_model::{
    TableRowNode, build_group_nodes, build_source_row_nodes, filter_source_row_nodes,
    filter_source_row_nodes_by_global_query, flatten_nodes, push_expanded_rows,
};
use rows::count_table_rows;

static NEXT_TABLE_ROWS_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Renderer-neutral scalar value used by table filtering and sorting.
#[derive(Debug, Clone, PartialEq)]
pub enum TableCellValue {
    /// No meaningful value is present.
    Empty,
    /// Text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
}

impl TableCellValue {
    /// Returns a stable string for filtering and debug output.
    pub fn filter_text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(value) => value.clone(),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }

    fn cmp_for_sort(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => left.total_cmp(right),
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Empty, Self::Empty) => Ordering::Equal,
            (Self::Empty, _) => Ordering::Less,
            (_, Self::Empty) => Ordering::Greater,
            _ => self.filter_text().cmp(&other.filter_text()),
        }
    }
}

impl Default for TableCellValue {
    fn default() -> Self {
        Self::Empty
    }
}

impl From<&str> for TableCellValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for TableCellValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<f64> for TableCellValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<i64> for TableCellValue {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<usize> for TableCellValue {
    fn from(value: usize) -> Self {
        Self::Number(value as f64)
    }
}

impl From<bool> for TableCellValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Renderer-neutral select option used by table select editors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSelectOption {
    value: String,
    label: String,
}

impl TableSelectOption {
    /// Creates a select option from a stable value and visible label.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns the stable option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Renderer-neutral cell editor kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditor {
    /// Single-line text editing with app-owned values.
    Text,
    /// Fixed-row multiline text editing with app-owned values.
    MultilineText {
        /// Fixed textarea row count requested by the column.
        rows: usize,
    },
    /// Boolean checkbox editing with app-owned values.
    Checkbox,
    /// Fixed-option select editing with app-owned values.
    Select,
}

impl TableCellEditor {
    /// Returns a normalized fixed row count for multiline text editors.
    pub const fn multiline(rows: usize) -> Self {
        Self::MultilineText {
            rows: normalize_table_multiline_editor_rows(rows),
        }
    }

    /// Returns a checkbox editor.
    pub const fn checkbox() -> Self {
        Self::Checkbox
    }

    /// Returns a select editor.
    pub const fn select() -> Self {
        Self::Select
    }

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::MultilineText { .. } => "multiline-text",
            Self::Checkbox => "checkbox",
            Self::Select => "select",
        }
    }

    /// Returns true when this editor preserves multiline values.
    pub const fn multiline_text(self) -> bool {
        matches!(self, Self::MultilineText { .. })
    }

    /// Returns the fixed textarea row count for multiline editors.
    pub const fn rows(self) -> Option<usize> {
        match self {
            Self::Text => None,
            Self::MultilineText { rows } => Some(rows),
            Self::Checkbox => None,
            Self::Select => None,
        }
    }
}

const fn normalize_table_multiline_editor_rows(rows: usize) -> usize {
    if rows == 0 { 1 } else { rows }
}

/// Renderer-neutral input state for table row-model resolution.
#[derive(Debug, Clone)]
pub struct TableState {
    column_tree: Vec<TableColumnNode>,
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_visibility: TableColumnVisibilityOverrides,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    row_pinning: TableRowPinning,
    row_pinning_policy: TableRowPinningPolicy,
    rows: Arc<[TableRow]>,
    rows_identity: u64,
    sorting: Vec<TableSort>,
    sorting_mode: TableStageMode,
    filters: Vec<TableFilter>,
    global_filter: Option<String>,
    filtering_mode: TableStageMode,
    faceting_mode: TableStageMode,
    manual_facets: Vec<TableColumnFacets>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    aggregation_fns: BTreeMap<String, TableAggregationFn>,
    expansion: TableExpansionState,
    expansion_mode: TableExpansionMode,
    selection_policy: TableSelectionPolicy,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl PartialEq for TableState {
    fn eq(&self, other: &Self) -> bool {
        self.column_tree == other.column_tree
            && self.columns == other.columns
            && self.column_order == other.column_order
            && self.column_visibility == other.column_visibility
            && self.column_pinning == other.column_pinning
            && self.column_sizing == other.column_sizing
            && self.row_pinning == other.row_pinning
            && self.row_pinning_policy == other.row_pinning_policy
            && self.rows.as_ref() == other.rows.as_ref()
            && self.sorting == other.sorting
            && self.sorting_mode == other.sorting_mode
            && self.filters == other.filters
            && self.global_filter == other.global_filter
            && self.filtering_mode == other.filtering_mode
            && self.faceting_mode == other.faceting_mode
            && self.manual_facets == other.manual_facets
            && self.grouping == other.grouping
            && self.aggregations == other.aggregations
            && self.aggregation_fns == other.aggregation_fns
            && self.expansion == other.expansion
            && self.expansion_mode == other.expansion_mode
            && self.selection_policy == other.selection_policy
            && self.selected_rows == other.selected_rows
            && self.pagination == other.pagination
    }
}

impl TableState {
    /// Creates table state from row descriptors.
    pub fn new(rows: impl IntoIterator<Item = TableRow>) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();

        Self {
            column_tree: Vec::new(),
            columns: Vec::new(),
            column_order: Vec::new(),
            column_visibility: TableColumnVisibilityOverrides::default(),
            column_pinning: TableColumnPinning::default(),
            column_sizing: TableColumnSizing::default(),
            row_pinning: TableRowPinning::default(),
            row_pinning_policy: TableRowPinningPolicy::default(),
            rows: rows.into(),
            rows_identity: next_table_rows_identity(),
            sorting: Vec::new(),
            sorting_mode: TableStageMode::default(),
            filters: Vec::new(),
            global_filter: None,
            filtering_mode: TableStageMode::default(),
            faceting_mode: TableStageMode::default(),
            manual_facets: Vec::new(),
            grouping: Vec::new(),
            aggregations: Vec::new(),
            aggregation_fns: BTreeMap::new(),
            expansion: TableExpansionState::default(),
            expansion_mode: TableExpansionMode::default(),
            selection_policy: TableSelectionPolicy::default(),
            selected_rows: BTreeSet::new(),
            pagination: TablePagination::default(),
        }
    }

    /// Applies column descriptors.
    pub fn with_columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        let (column_tree, columns) =
            normalize_table_column_tree(columns.into_iter().map(TableColumnNode::from));
        self.column_tree = column_tree;
        self.columns = columns;
        self
    }

    /// Applies nested column-tree descriptors.
    pub fn with_column_tree<N>(mut self, column_tree: impl IntoIterator<Item = N>) -> Self
    where
        N: Into<TableColumnNode>,
    {
        let (column_tree, columns) = normalize_table_column_tree(column_tree);
        self.column_tree = column_tree;
        self.columns = columns;
        self
    }

    /// Replaces source rows while preserving the rest of the table configuration.
    pub fn with_rows(mut self, rows: impl IntoIterator<Item = TableRow>) -> Self {
        self.rows = rows.into_iter().collect::<Vec<_>>().into();
        self.rows_identity = next_table_rows_identity();
        self
    }

    /// Applies explicit column order.
    pub fn with_column_order(
        mut self,
        column_order: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.column_order = column_order.into_iter().map(Into::into).collect();
        self
    }

    /// Applies runtime column visibility overrides.
    pub fn with_column_visibility(
        mut self,
        column_visibility: TableColumnVisibilityOverrides,
    ) -> Self {
        self.column_visibility = column_visibility;
        self
    }

    /// Applies pinned column state.
    pub fn with_column_pinning(mut self, column_pinning: TableColumnPinning) -> Self {
        self.column_pinning = column_pinning;
        self
    }

    /// Applies pinned row state.
    pub fn with_row_pinning(mut self, row_pinning: TableRowPinning) -> Self {
        self.row_pinning = row_pinning;
        self
    }

    /// Applies the pinned row visibility policy.
    pub const fn with_row_pinning_policy(
        mut self,
        row_pinning_policy: TableRowPinningPolicy,
    ) -> Self {
        self.row_pinning_policy = row_pinning_policy;
        self
    }

    /// Applies committed column sizing state.
    pub fn with_column_sizing(mut self, column_sizing: TableColumnSizing) -> Self {
        self.column_sizing = column_sizing;
        self
    }

    /// Applies sort specifications.
    pub fn with_sorting(mut self, sorting: impl IntoIterator<Item = TableSort>) -> Self {
        self.sorting = sorting.into_iter().collect();
        self
    }

    /// Applies sorting ownership mode.
    pub const fn with_sorting_mode(mut self, sorting_mode: TableStageMode) -> Self {
        self.sorting_mode = sorting_mode;
        self
    }

    /// Marks sorting as caller-owned.
    pub const fn with_manual_sorting(mut self) -> Self {
        self.sorting_mode = TableStageMode::Manual;
        self
    }

    /// Applies filter specifications.
    pub fn with_filters(mut self, filters: impl IntoIterator<Item = TableFilter>) -> Self {
        self.filters = filters.into_iter().collect();
        self
    }

    /// Applies a global text filter query.
    ///
    /// Empty or whitespace-only queries clear the global filter state.
    pub fn with_global_filter(mut self, query: impl Into<String>) -> Self {
        self.global_filter = normalize_table_global_filter_query(query);
        self
    }

    /// Clears the global text filter query.
    pub fn without_global_filter(mut self) -> Self {
        self.global_filter = None;
        self
    }

    /// Applies filtering ownership mode.
    pub const fn with_filtering_mode(mut self, filtering_mode: TableStageMode) -> Self {
        self.filtering_mode = filtering_mode;
        self
    }

    /// Marks filtering as caller-owned.
    pub const fn with_manual_filtering(mut self) -> Self {
        self.filtering_mode = TableStageMode::Manual;
        self
    }

    /// Applies faceting ownership mode.
    pub const fn with_faceting_mode(mut self, faceting_mode: TableStageMode) -> Self {
        self.faceting_mode = faceting_mode;
        self
    }

    /// Marks faceting as caller-owned.
    pub const fn with_manual_faceting(mut self) -> Self {
        self.faceting_mode = TableStageMode::Manual;
        self
    }

    /// Applies caller-owned facet payloads keyed by column id.
    pub fn with_manual_facets(
        mut self,
        facets: impl IntoIterator<Item = TableColumnFacets>,
    ) -> Self {
        let mut facets_by_column = BTreeMap::new();
        for facet in facets {
            facets_by_column.insert(
                facet.column().clone(),
                facet.with_mode(TableStageMode::Manual),
            );
        }
        self.manual_facets = facets_by_column.into_values().collect();
        self
    }

    /// Applies grouping column ids in outer-to-inner order.
    pub fn with_grouping(
        mut self,
        grouping: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        let mut seen = BTreeSet::new();
        self.grouping = grouping
            .into_iter()
            .map(Into::into)
            .filter(|column| seen.insert(column.clone()))
            .collect();
        self
    }

    /// Applies aggregate specifications keyed by column id.
    pub fn with_aggregations(
        mut self,
        aggregations: impl IntoIterator<Item = TableAggregation>,
    ) -> Self {
        let mut aggregations_by_column = BTreeMap::new();
        for aggregation in aggregations {
            aggregations_by_column.insert(aggregation.column().clone(), aggregation);
        }
        self.aggregations = aggregations_by_column.into_values().collect();
        self
    }

    /// Registers a named aggregation callback.
    pub fn with_aggregation_fn(
        mut self,
        name: impl Into<String>,
        aggregation_fn: impl Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.aggregation_fns
            .insert(name.into(), TableAggregationFn::new(aggregation_fn));
        self
    }

    /// Applies explicit expanded group row ids.
    pub fn with_expanded_rows(
        mut self,
        expanded_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.expansion = TableExpansionState::rows(expanded_rows);
        self
    }

    /// Applies the expansion mode where every group row is expanded.
    pub fn with_all_rows_expanded(mut self) -> Self {
        self.expansion = TableExpansionState::All;
        self
    }

    /// Applies expansion behavior for source-tree row models.
    pub const fn with_expansion_mode(mut self, expansion_mode: TableExpansionMode) -> Self {
        self.expansion_mode = expansion_mode;
        self
    }

    /// Lets callers provide the visible source-tree snapshot directly.
    pub const fn with_manual_expansion(mut self) -> Self {
        self.expansion_mode = TableExpansionMode::Manual;
        self
    }

    /// Applies the row-selection policy.
    pub fn with_selection_policy(mut self, selection_policy: TableSelectionPolicy) -> Self {
        self.selection_policy = selection_policy;
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies the selection cardinality.
    pub fn with_selection_mode(mut self, selection_mode: TableSelectionMode) -> Self {
        self.selection_policy = self.selection_policy.with_selection_mode(selection_mode);
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies the selection activation mode.
    pub const fn with_selection_activation_mode(
        mut self,
        activation_mode: TableSelectionActivationMode,
    ) -> Self {
        self.selection_policy = self.selection_policy.with_activation_mode(activation_mode);
        self
    }

    /// Applies the sub-row selection policy.
    pub fn with_sub_row_selection_policy(
        mut self,
        sub_row_policy: TableSubRowSelectionPolicy,
    ) -> Self {
        self.selection_policy = self.selection_policy.with_sub_row_policy(sub_row_policy);
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies selected row ids.
    pub fn with_selected_rows(
        mut self,
        selected_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.selected_rows = self.selection_policy.normalize_selected_rows(selected_rows);
        self
    }

    /// Applies pagination state.
    pub const fn with_pagination(mut self, pagination: TablePagination) -> Self {
        self.pagination = pagination;
        self
    }

    /// Returns configured columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns the normalized configured column tree.
    pub fn column_tree(&self) -> &[TableColumnNode] {
        &self.column_tree
    }

    /// Returns explicit column order ids.
    pub fn column_order(&self) -> &[TableColumnId] {
        &self.column_order
    }

    /// Returns runtime column visibility overrides.
    pub const fn column_visibility(&self) -> &TableColumnVisibilityOverrides {
        &self.column_visibility
    }

    /// Returns source rows.
    pub fn rows(&self) -> &[TableRow] {
        self.rows.as_ref()
    }

    /// Returns sort specifications.
    pub fn sorting(&self) -> &[TableSort] {
        &self.sorting
    }

    /// Returns the sorting ownership mode.
    pub const fn sorting_mode(&self) -> TableStageMode {
        self.sorting_mode
    }

    /// Returns filter specifications.
    pub fn filters(&self) -> &[TableFilter] {
        &self.filters
    }

    /// Returns the normalized global text filter query.
    pub fn global_filter(&self) -> Option<&str> {
        self.global_filter.as_deref()
    }

    /// Returns the filtering ownership mode.
    pub const fn filtering_mode(&self) -> TableStageMode {
        self.filtering_mode
    }

    /// Returns the faceting ownership mode.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns caller-owned facet payloads.
    pub fn manual_facets(&self) -> &[TableColumnFacets] {
        &self.manual_facets
    }

    /// Returns grouping column ids in outer-to-inner order.
    pub fn grouping(&self) -> &[TableColumnId] {
        &self.grouping
    }

    /// Returns aggregate specifications keyed by column id.
    pub fn aggregations(&self) -> &[TableAggregation] {
        &self.aggregations
    }

    /// Returns the number of named aggregation callbacks.
    pub fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fns.len()
    }

    /// Returns whether a named aggregation callback has been registered.
    pub fn has_aggregation_fn(&self, name: &str) -> bool {
        self.aggregation_fns.contains_key(name)
    }

    /// Returns pinned column state.
    pub const fn column_pinning(&self) -> &TableColumnPinning {
        &self.column_pinning
    }

    /// Returns pinned row state.
    pub const fn row_pinning(&self) -> &TableRowPinning {
        &self.row_pinning
    }

    /// Returns the pinned row visibility policy.
    pub const fn row_pinning_policy(&self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns committed column sizing state.
    pub const fn column_sizing(&self) -> &TableColumnSizing {
        &self.column_sizing
    }

    /// Returns caller-owned row expansion state.
    pub const fn expansion(&self) -> &TableExpansionState {
        &self.expansion
    }

    /// Returns source-tree row expansion behavior.
    pub const fn expansion_mode(&self) -> TableExpansionMode {
        self.expansion_mode
    }

    /// Returns the selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns selected row ids.
    pub const fn selected_rows(&self) -> &BTreeSet<TableRowId> {
        &self.selected_rows
    }

    /// Returns pagination state.
    pub const fn pagination(&self) -> TablePagination {
        self.pagination
    }

    /// Returns a cheap identity key for runtime row-model caches.
    ///
    /// The key is conservative: cloned states share the row identity, while newly
    /// constructed states get a new identity even when their row contents match.
    pub fn cache_key(&self) -> TableStateCacheKey {
        TableStateCacheKey {
            rows_identity: self.rows_identity,
            row_count: count_table_rows(&self.rows),
            column_tree: self.column_tree.clone(),
            columns: self.columns.clone(),
            column_order: self.column_order.clone(),
            column_visibility: self.column_visibility.clone(),
            column_pinning: self.column_pinning.clone(),
            column_sizing: self.column_sizing.clone(),
            row_pinning: self.row_pinning.clone(),
            row_pinning_policy: self.row_pinning_policy,
            sorting: self.sorting.clone(),
            sorting_mode: self.sorting_mode,
            filters: self.filters.clone(),
            global_filter: self.global_filter.clone(),
            filtering_mode: self.filtering_mode,
            faceting_mode: self.faceting_mode,
            manual_facets: self.manual_facets.clone(),
            grouping: self.grouping.clone(),
            aggregations: self.aggregations.clone(),
            aggregation_fns: self.aggregation_fns.clone(),
            expansion: self.expansion.clone(),
            expansion_mode: self.expansion_mode,
            selection_policy: self.selection_policy,
            selected_rows: self.selected_rows.clone(),
            pagination: self.pagination,
        }
    }

    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> Vec<TableColumn> {
        self.visible_column_regions().flattened()
    }

    /// Returns visible columns split into pinned regions.
    pub fn visible_column_regions(&self) -> TableColumnRegions {
        TableColumnRegions::from_visible_columns(
            self.ordered_visible_columns(),
            &self.column_pinning,
        )
    }

    fn ordered_visible_columns(&self) -> Vec<TableColumn> {
        if self.column_order.is_empty() {
            return self
                .columns
                .iter()
                .filter(|column| self.column_visibility.is_visible(column))
                .cloned()
                .collect();
        }

        let columns_by_id: BTreeMap<_, _> = self
            .columns
            .iter()
            .map(|column| (column.id().clone(), column.clone()))
            .collect();

        self.column_order
            .iter()
            .filter_map(|id| columns_by_id.get(id))
            .filter(|column| self.column_visibility.is_visible(column))
            .cloned()
            .collect()
    }

    /// Resolves row models from the input state.
    pub fn resolve(&self) -> TableResolvedState {
        let mut duplicate_row_ids = BTreeSet::new();
        let mut seen_row_ids = BTreeSet::new();
        record_source_row_ids(&self.rows, &mut seen_row_ids, &mut duplicate_row_ids);
        let include_source_children = self.grouping.is_empty();
        let global_filterable_columns = self.global_filterable_column_ids();
        let selected_rows = self
            .selection_policy
            .resolve_selected_rows(&self.rows, &self.selected_rows);
        let mut source_index = 0;
        let source_nodes = build_source_row_nodes(
            &self.rows,
            &selected_rows,
            &self.expansion,
            include_source_children,
            None,
            0,
            &mut source_index,
        );
        let core_rows = flatten_nodes(&source_nodes);
        let column_filtered_nodes = if self.filtering_mode.is_manual() {
            source_nodes.clone()
        } else {
            filter_source_row_nodes(&source_nodes, &self.filters, None)
        };
        let global_filtered_nodes = if self.filtering_mode.is_manual() {
            column_filtered_nodes.clone()
        } else {
            filter_source_row_nodes_by_global_query(
                &column_filtered_nodes,
                self.global_filter.as_deref(),
                &global_filterable_columns,
            )
        };
        let column_facets = self.resolve_column_facets(&source_nodes, &global_filterable_columns);
        let global_facet_summary =
            self.resolve_global_facet_summary(&column_filtered_nodes, &global_filterable_columns);

        let core_model = TableRowModel::new(TableRowModelStage::Core, core_rows);

        let filtered_rows = flatten_nodes(&global_filtered_nodes);
        let filtered_model = TableRowModel::new(TableRowModelStage::Filtered, filtered_rows);

        let grouped_nodes = if self.grouping.is_empty() {
            global_filtered_nodes
        } else {
            self.group_nodes(filtered_model.rows())
        };
        let grouped_rows = flatten_nodes(&grouped_nodes);
        let grouped_model = TableRowModel::new(TableRowModelStage::Grouped, grouped_rows);

        let sorted_nodes = if self.sorting_mode.is_manual() {
            grouped_nodes.clone()
        } else {
            self.sort_nodes(grouped_nodes)
        };
        let sorted_rows = flatten_nodes(&sorted_nodes);
        let sorted_model = TableRowModel::new(TableRowModelStage::Sorted, sorted_rows);

        let expanded_rows = self.expand_nodes(&sorted_nodes);
        let expanded_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Expanded,
            expanded_rows,
            sorted_model.rows().to_vec(),
        );

        let paginated_model = TableRowModel::new(
            TableRowModelStage::Paginated,
            self.pagination.apply(expanded_model.rows()),
        );
        let row_regions = TableRowRegions::from_models(
            expanded_model.rows(),
            paginated_model.rows(),
            &self.row_pinning,
            self.row_pinning_policy,
        );
        let final_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Final,
            row_regions.flattened(),
            expanded_model.rows_by_id().values().cloned(),
        );

        let visible_column_regions = self.visible_column_regions();
        let visible_column_sizing = TableResolvedColumnSizingRegions::from_column_regions(
            &visible_column_regions,
            &self.column_sizing,
        );
        let header_groups = TableResolvedHeaderGroupRegions::from_column_tree(
            &self.column_tree,
            &visible_column_regions,
        );

        TableResolvedState {
            visible_columns: visible_column_regions.flattened(),
            visible_column_regions,
            visible_column_sizing,
            header_groups,
            duplicate_row_ids: duplicate_row_ids.into_iter().collect(),
            faceting_mode: self.faceting_mode,
            column_facets,
            global_facet_summary,
            row_pinning_policy: self.row_pinning_policy,
            row_regions,
            core_model,
            filtered_model,
            grouped_model,
            sorted_model,
            expanded_model,
            paginated_model,
            final_model,
            selection_policy: self.selection_policy,
        }
    }

    fn compare_rows(&self, left: &TableResolvedRow, right: &TableResolvedRow) -> Ordering {
        if self.sorting.is_empty() {
            return Ordering::Equal;
        }

        for sort in &self.sorting {
            let left_value = left.cell(sort.column()).cloned().unwrap_or_default();
            let right_value = right.cell(sort.column()).cloned().unwrap_or_default();
            let ordering = left_value.cmp_for_sort(&right_value);
            let ordering = match sort.direction() {
                TableSortDirection::Ascending => ordering,
                TableSortDirection::Descending => ordering.reverse(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        left.id().cmp(right.id())
    }

    fn group_nodes(&self, rows: &[TableResolvedRow]) -> Vec<TableRowNode> {
        if self.grouping.is_empty() {
            return rows
                .iter()
                .cloned()
                .map(TableRowNode::leaf)
                .collect::<Vec<_>>();
        }

        build_group_nodes(
            rows,
            &self.grouping,
            &self.aggregations,
            &self.aggregation_fns,
            0,
            None,
            None,
        )
    }

    fn sort_nodes(&self, mut nodes: Vec<TableRowNode>) -> Vec<TableRowNode> {
        for node in &mut nodes {
            node.children = self.sort_nodes(std::mem::take(&mut node.children));
        }

        if !self.sorting.is_empty() {
            nodes.sort_by(|left, right| self.compare_rows(&left.row, &right.row));
        }

        nodes
    }

    fn expand_nodes(&self, nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
        if self.grouping.is_empty() && !self.expansion_mode.prunes_collapsed_rows() {
            return flatten_nodes(nodes);
        }

        let mut rows = Vec::new();
        for node in nodes {
            push_expanded_rows(node, &self.expansion, &mut rows);
        }
        rows
    }

    fn resolve_column_facets(
        &self,
        source_nodes: &[TableRowNode],
        global_filterable_columns: &[TableColumnId],
    ) -> Vec<TableColumnFacets> {
        self.columns
            .iter()
            .filter_map(|column| {
                if let Some(manual) = self
                    .manual_facets
                    .iter()
                    .find(|facet| facet.column() == column.id())
                {
                    return Some(manual.clone());
                }

                if self.faceting_mode.is_manual() {
                    return None;
                }

                Some(resolve_client_column_facets(
                    column.id().clone(),
                    source_nodes,
                    &self.filters,
                    self.global_filter.as_deref(),
                    global_filterable_columns,
                    self.filtering_mode,
                ))
            })
            .collect()
    }

    fn resolve_global_facet_summary(
        &self,
        source_nodes: &[TableRowNode],
        global_filterable_columns: &[TableColumnId],
    ) -> TableGlobalFacetSummary {
        if self.faceting_mode.is_manual() {
            return TableGlobalFacetSummary::manual();
        }

        let row_count = count_table_row_nodes(source_nodes);
        let mut column_facets = Vec::new();
        for column_id in global_filterable_columns {
            column_facets.push(resolve_client_global_column_facets(
                column_id.clone(),
                source_nodes,
            ));
        }

        TableGlobalFacetSummary::client(row_count, column_facets)
    }

    fn global_filterable_column_ids(&self) -> Vec<TableColumnId> {
        self.columns
            .iter()
            .filter(|column| column.global_filterable())
            .map(|column| column.id().clone())
            .collect()
    }
}

/// Cheap invalidation key for runtime caches of resolved table row models.
#[derive(Debug, Clone, PartialEq)]
pub struct TableStateCacheKey {
    rows_identity: u64,
    row_count: usize,
    column_tree: Vec<TableColumnNode>,
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_visibility: TableColumnVisibilityOverrides,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    row_pinning: TableRowPinning,
    row_pinning_policy: TableRowPinningPolicy,
    sorting: Vec<TableSort>,
    sorting_mode: TableStageMode,
    filters: Vec<TableFilter>,
    global_filter: Option<String>,
    filtering_mode: TableStageMode,
    faceting_mode: TableStageMode,
    manual_facets: Vec<TableColumnFacets>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    aggregation_fns: BTreeMap<String, TableAggregationFn>,
    expansion: TableExpansionState,
    expansion_mode: TableExpansionMode,
    selection_policy: TableSelectionPolicy,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl TableStateCacheKey {
    /// Returns the opaque identity assigned to this state's row storage.
    pub const fn rows_identity(&self) -> u64 {
        self.rows_identity
    }

    /// Returns the number of source rows covered by this cache key.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }
}

fn next_table_rows_identity() -> u64 {
    NEXT_TABLE_ROWS_IDENTITY.fetch_add(1, AtomicOrdering::Relaxed)
}

fn count_table_row_nodes(nodes: &[TableRowNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_table_row_nodes(&node.children))
        .sum()
}

fn record_source_row_ids(
    rows: &[TableRow],
    seen: &mut BTreeSet<TableRowId>,
    duplicates: &mut BTreeSet<TableRowId>,
) {
    for row in rows {
        if !seen.insert(row.id().clone()) {
            duplicates.insert(row.id().clone());
        }
        record_source_row_ids(row.children(), seen, duplicates);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_text_editor_rows_are_normalized() {
        let column = TableColumn::new("notes", "Notes").with_multiline_text_editor(0);

        assert_eq!(
            column.editor(),
            Some(TableCellEditor::MultilineText { rows: 1 })
        );
        assert!(column.text_editable());
        assert_eq!(TableCellEditor::multiline(4).rows(), Some(4));
        assert!(TableCellEditor::multiline(4).multiline_text());
    }

    #[test]
    fn checkbox_editor_is_exposed_as_a_stable_variant() {
        let column = TableColumn::new("enabled", "Enabled").with_checkbox_editor();

        assert_eq!(column.editor(), Some(TableCellEditor::Checkbox));
        assert!(column.text_editable());
        assert_eq!(TableCellEditor::checkbox().as_str(), "checkbox");
    }

    #[test]
    fn select_editor_exposes_options_and_stable_labels() {
        let column = TableColumn::new("status", "Status").with_select_editor([
            TableSelectOption::new("ready", "Ready"),
            TableSelectOption::new("blocked", "Blocked"),
        ]);

        assert_eq!(column.editor(), Some(TableCellEditor::Select));
        assert_eq!(column.select_options().len(), 2);
        assert_eq!(column.select_options()[0].value(), "ready");
        assert_eq!(column.select_options()[0].label(), "Ready");
        assert_eq!(TableCellEditor::select().as_str(), "select");
        assert_eq!(TableCellEditor::select().rows(), None);
    }

    fn sample_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-b")
                .with_cell("name", "Beta")
                .with_cell("team", "ops")
                .with_cell("score", 20_usize),
            TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "design")
                .with_cell("score", 10_usize),
            TableRow::new("row-c")
                .with_cell("name", "Gamma")
                .with_cell("team", "ops")
                .with_cell("score", 30_usize),
        ]
    }

    fn aggregate_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-1")
                .with_cell("team", "ops")
                .with_cell("name", "Alpha")
                .with_cell("score", 20_usize)
                .with_cell("low", 4_usize)
                .with_cell("high", 11_usize)
                .with_cell("duration", 2_usize)
                .with_cell("noise", "n/a"),
            TableRow::new("row-2")
                .with_cell("team", "ops")
                .with_cell("name", "Beta")
                .with_cell("score", 30_usize)
                .with_cell("low", 2_usize)
                .with_cell("high", 9_usize)
                .with_cell("duration", 4_usize)
                .with_cell("noise", "unknown"),
            TableRow::new("row-3")
                .with_cell("team", "design")
                .with_cell("name", "Gamma")
                .with_cell("score", 7_usize)
                .with_cell("low", 7_usize)
                .with_cell("high", 7_usize)
                .with_cell("duration", 10_usize)
                .with_cell("noise", "unknown"),
        ]
    }

    fn tree_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("pkg")
                .with_cell("name", "Workspace")
                .with_cell("team", "core")
                .with_cell("score", 100_usize)
                .with_child(
                    TableRow::new("pkg-ui")
                        .with_cell("name", "UI")
                        .with_cell("team", "ui")
                        .with_cell("score", 30_usize),
                )
                .with_child(
                    TableRow::new("pkg-core")
                        .with_cell("name", "Core")
                        .with_cell("team", "core")
                        .with_cell("score", 70_usize)
                        .with_child(
                            TableRow::new("pkg-core-test")
                                .with_cell("name", "Core Test")
                                .with_cell("team", "core")
                                .with_cell("score", 10_usize),
                        ),
                ),
            TableRow::new("docs")
                .with_cell("name", "Docs")
                .with_cell("team", "docs")
                .with_cell("score", 20_usize),
        ]
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

    fn row_ids(rows: &[TableResolvedRow]) -> Vec<&str> {
        rows.iter().map(|row| row.id().as_str()).collect()
    }

    #[test]
    fn row_model_pipeline_names_full_and_v0_stages() {
        assert_eq!(
            TABLE_ROW_MODEL_PIPELINE.map(TableRowModelStage::as_str),
            [
                "core",
                "filtered",
                "grouped",
                "sorted",
                "expanded",
                "paginated",
                "final"
            ]
        );
        assert_eq!(
            TABLE_ROW_MODEL_V0_PIPELINE.map(TableRowModelStage::as_str),
            ["core", "filtered", "sorted", "paginated", "final"]
        );
        assert!(!TableRowModelStage::Grouped.implemented_in_v0());
        assert!(!TableRowModelStage::Expanded.implemented_in_v0());
        assert!(TableRowModelStage::Sorted.implemented_in_v0());
    }

    #[test]
    fn column_widths_resolve_from_defaults_and_committed_sizing() {
        let column = TableColumn::new("name", "Name")
            .with_width(ui_px(120.0))
            .with_min_width(ui_px(80.0))
            .with_max_width(ui_px(160.0));
        let content_fit = TableColumn::new("status", "Status").with_content_fit();

        assert_eq!(column.width(), ui_px(120.0));
        assert_eq!(column.min_width(), ui_px(80.0));
        assert_eq!(column.max_width(), ui_px(160.0));
        assert_eq!(
            content_fit.width_policy(),
            TableColumnWidthPolicy::ContentFit
        );
        assert!(content_fit.is_content_fit());
        assert!(column.resizable());
        assert_eq!(
            column.resolved_width(&TableColumnSizing::new()),
            ui_px(120.0),
            "without committed sizing, the preferred width is used"
        );

        let undersized = TableColumnSizing::new().with_width("name", ui_px(40.0));
        assert_eq!(
            column.resolved_width(&undersized),
            ui_px(80.0),
            "committed widths are clamped to the column minimum"
        );

        let oversized = TableColumnSizing::new().with_width("name", ui_px(220.0));
        assert_eq!(
            column.resolved_width(&oversized),
            ui_px(160.0),
            "committed widths are clamped to the column maximum"
        );

        let unrelated = TableColumnSizing::new().with_width("team", ui_px(140.0));
        assert_eq!(
            column.resolved_width(&unrelated),
            ui_px(120.0),
            "unknown committed sizing ids do not affect this column"
        );
        assert_eq!(
            content_fit.resolved_width(&TableColumnSizing::new()),
            TABLE_DEFAULT_COLUMN_WIDTH,
            "content-fit columns still resolve to their configured fallback until the adapter overlays measured width"
        );
    }

    #[test]
    fn flat_columns_project_to_a_flat_column_tree() {
        let state = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
        ]);

        assert_eq!(
            state
                .columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "team"]
        );
        assert_eq!(
            state
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "team"]
        );
        assert_eq!(state.column_tree().len(), 2);
        assert!(state.column_tree().iter().all(TableColumnNode::is_column));
        assert_eq!(
            state.column_tree()[0]
                .as_column()
                .expect("flat tree should keep the first leaf")
                .label(),
            "Name"
        );
    }

    #[test]
    fn column_tree_projects_nested_leaves_and_prunes_duplicates() {
        let state = TableState::new(sample_rows()).with_column_tree([
            TableColumnNode::group(TableColumnGroup::new(
                "identity",
                "Identity",
                [
                    TableColumnNode::column(TableColumn::new("name", "Name")),
                    TableColumnNode::group(TableColumnGroup::new(
                        "metrics",
                        "Metrics",
                        [
                            TableColumnNode::column(TableColumn::new("score", "Score")),
                            TableColumnNode::column(TableColumn::new("score", "Duplicate score")),
                        ],
                    )),
                ],
            )),
            TableColumnNode::column(TableColumn::new("team", "Team")),
            TableColumnNode::group(TableColumnGroup::new(
                "duplicate-only",
                "Duplicate only",
                [TableColumnNode::column(TableColumn::new(
                    "name",
                    "Duplicate name",
                ))],
            )),
        ]);

        assert_eq!(
            state
                .columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "score", "team"],
            "leaf projection follows first-seen tree order"
        );
        assert_eq!(
            state
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "score", "team"],
            "group ids do not become behavior columns"
        );
        assert_eq!(
            state.column_tree().len(),
            2,
            "groups with no non-duplicate leaves are removed"
        );

        let identity = state.column_tree()[0]
            .as_group()
            .expect("first root node should remain a group");
        assert_eq!(identity.id().as_str(), "identity");
        assert_eq!(identity.label(), "Identity");
        assert_eq!(identity.children().len(), 2);
        assert_eq!(
            identity.children()[0]
                .as_column()
                .expect("first group child should be a leaf")
                .id()
                .as_str(),
            "name"
        );

        let metrics = identity.children()[1]
            .as_group()
            .expect("second group child should stay nested");
        assert_eq!(metrics.id().as_str(), "metrics");
        assert_eq!(metrics.label(), "Metrics");
        assert_eq!(metrics.children().len(), 1);
        assert_eq!(
            metrics.children()[0]
                .as_column()
                .expect("duplicate metric leaf should be pruned")
                .id()
                .as_str(),
            "score"
        );
    }

    #[test]
    fn column_tree_participates_in_table_cache_key() {
        let base = TableState::new(sample_rows());
        let flat = base.clone().with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
        ]);
        let grouped = base.with_column_tree([TableColumnGroup::new(
            "identity",
            "Identity",
            [
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team"),
            ],
        )]);

        assert_eq!(
            flat.columns(),
            grouped.columns(),
            "grouped trees keep the same leaf-column contract"
        );
        assert_ne!(
            flat.cache_key(),
            grouped.cache_key(),
            "tree shape invalidates header caches even when leaf columns match"
        );
    }

    #[test]
    fn content_fit_width_policy_participates_in_cache_key() {
        let base = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("status", "Status"),
        ]);
        let content_fit = base.clone().with_columns([
            TableColumn::new("name", "Name").with_content_fit(),
            TableColumn::new("status", "Status"),
        ]);

        assert_eq!(
            base.columns()[0].width_policy(),
            TableColumnWidthPolicy::Fixed
        );
        assert_eq!(
            content_fit.columns()[0].width_policy(),
            TableColumnWidthPolicy::ContentFit
        );
        assert_ne!(
            base.cache_key(),
            content_fit.cache_key(),
            "content-fit policy should invalidate the table cache key"
        );
    }

    #[test]
    fn sizing_state_keeps_unknown_ids_without_changing_visible_columns() {
        let state = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
            ])
            .with_column_sizing(TableColumnSizing::from_widths([
                ("team", ui_px(320.0)),
                ("missing", ui_px(480.0)),
            ]));

        let visible_columns = state.visible_columns();
        assert_eq!(
            visible_columns
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            visible_columns[0].resolved_width(state.column_sizing()),
            TABLE_DEFAULT_COLUMN_WIDTH,
            "hidden and unknown sizing entries do not contribute visible widths"
        );
        assert_eq!(
            state.column_sizing().width(&TableColumnId::new("missing")),
            Some(ui_px(480.0)),
            "unknown ids remain caller-owned state instead of being silently pruned"
        );
    }

    #[test]
    fn resolved_column_sizing_tracks_region_offsets_and_totals() {
        let resolved = TableState::new(sample_rows())
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
            .resolve();
        let sizing = resolved.visible_column_sizing();

        assert_eq!(sizing.left_width(), ui_px(190.0));
        assert_eq!(sizing.center_width(), ui_px(120.0));
        assert_eq!(sizing.right_width(), ui_px(60.0));
        assert_eq!(sizing.total_width(), ui_px(370.0));
        assert_eq!(sizing.region_width(TableColumnRegion::Left), ui_px(190.0));
        assert_eq!(
            sizing
                .left()
                .iter()
                .map(|column| {
                    (
                        column.column_id().as_str(),
                        column.width(),
                        column.start(),
                        column.after(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("score", ui_px(90.0), ui_px(0.0), ui_px(100.0)),
                ("name", ui_px(100.0), ui_px(90.0), ui_px(0.0)),
            ],
            "left region offsets follow resolved visible order and clamp committed widths"
        );

        let status = sizing
            .column(&TableColumnId::new("status"))
            .expect("status sizing should resolve");
        assert_eq!(status.region(), TableColumnRegion::Right);
        assert_eq!(status.width(), ui_px(60.0));
        assert_eq!(status.start(), ui_px(0.0));
        assert_eq!(status.after(), ui_px(0.0));
        assert!(!status.resizable());
    }

    #[test]
    fn resolved_column_sizing_is_stable_across_row_model_changes() {
        let base = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
                TableColumn::new("score", "Score").with_width(ui_px(80.0)),
            ])
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(96.0)));

        let base_sizing = base.resolve().visible_column_sizing().clone();
        let changed_rows = base
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["row-a"])
            .with_pagination(TablePagination::new(0, 1))
            .resolve()
            .visible_column_sizing()
            .clone();

        assert_eq!(base_sizing, changed_rows);
    }

    #[test]
    fn column_resize_on_end_commits_only_when_finished() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnEnd,
            TableColumnResizeDirection::Ltr,
            &sizing,
            &resize,
            ui_px(60.0),
        );
        assert!(moved.committed_sizing().is_none());
        assert_eq!(moved.state().delta_offset(), Some(ui_px(50.0)));
        assert_eq!(moved.state().delta_percentage(), Some(0.5));
        assert_eq!(
            moved.state().preview_width(&TableColumnId::new("name")),
            Some(ui_px(150.0))
        );

        let ended = end_table_column_resize(
            TableColumnResizeMode::OnEnd,
            TableColumnResizeDirection::Ltr,
            &sizing,
            moved.state(),
            Some(ui_px(60.0)),
        );
        assert!(!ended.state().is_resizing());
        assert_eq!(
            ended
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );
    }

    #[test]
    fn column_resize_on_change_commits_during_drag_and_resets_on_end() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Ltr,
            &sizing,
            &resize,
            ui_px(60.0),
        );
        assert_eq!(
            moved
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );

        let ended = end_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Ltr,
            &sizing,
            moved.state(),
            Some(ui_px(60.0)),
        );
        assert!(!ended.state().is_resizing());
        assert_eq!(
            ended
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );
    }

    #[test]
    fn column_resize_rtl_flips_pointer_delta() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Rtl,
            &sizing,
            &resize,
            ui_px(60.0),
        );

        assert_eq!(moved.state().delta_offset(), Some(ui_px(-50.0)));
        assert_eq!(
            moved
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(50.0))
        );
    }

    #[test]
    fn stable_row_ids_survive_filtering_sorting_and_pagination() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::new(0, 1))
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-c"]
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c", "row-b"]
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c"]
        );
        assert!(
            resolved
                .core_model()
                .row(&TableRowId::new("row-b"))
                .is_some()
        );
    }

    #[test]
    fn global_filter_matches_globally_filterable_columns_and_normalizes_queries() {
        let base = TableState::new([
            TableRow::new("row-1")
                .with_cell("name", "Alpha")
                .with_cell("notes", "done")
                .with_cell("score", 1_usize),
            TableRow::new("row-2")
                .with_cell("name", "Beta")
                .with_cell("notes", "pending")
                .with_cell("score", 2_usize),
            TableRow::new("row-3")
                .with_cell("name", "Done")
                .with_cell("notes", "pending")
                .with_cell("score", 3_usize),
        ])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("notes", "Notes").with_global_filterable(false),
            TableColumn::new("score", "Score"),
        ]);

        assert_eq!(base.clone().with_global_filter("   ").global_filter(), None);
        assert_eq!(
            base.clone().with_global_filter("  done  ").global_filter(),
            Some("done")
        );
        assert_eq!(
            base.cache_key(),
            base.clone().with_global_filter("   ").cache_key(),
            "whitespace-only global filters normalize to cache-key absence"
        );
        assert_ne!(
            base.cache_key(),
            base.clone().with_global_filter("done").cache_key(),
            "non-empty global filters should invalidate row-model caches"
        );

        let resolved = base.with_global_filter("done").resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-3"],
            "global filtering should ignore columns opted out of the global scan"
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-3"],
            "global filtering should feed the final row model"
        );
    }

    #[test]
    fn global_filter_runs_after_column_filters_before_sorting_and_pagination() {
        let resolved = TableState::new([
            TableRow::new("row-1")
                .with_cell("team", "UI")
                .with_cell("name", "Done Alpha")
                .with_cell("score", 40_usize),
            TableRow::new("row-2")
                .with_cell("team", "UI")
                .with_cell("name", "Done Beta")
                .with_cell("score", 10_usize),
            TableRow::new("row-3")
                .with_cell("team", "UI")
                .with_cell("name", "Gamma")
                .with_cell("score", 30_usize),
            TableRow::new("row-4")
                .with_cell("team", "API")
                .with_cell("name", "Done Delta")
                .with_cell("score", 20_usize),
        ])
        .with_columns([
            TableColumn::new("team", "Team"),
            TableColumn::new("name", "Name"),
            TableColumn::new("score", "Score"),
        ])
        .with_filters([TableFilter::contains("team", "UI")])
        .with_global_filter("done")
        .with_sorting([TableSort::descending("score")])
        .with_pagination(TablePagination::new(0, 1))
        .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1", "row-2"],
            "global filtering should run after column filtering"
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1", "row-2"],
            "sorting should still see the globally filtered row set"
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1"],
            "pagination should apply after global filtering and sorting"
        );
    }

    #[test]
    fn categorical_filters_match_exact_tokens_and_multiple_values() {
        let resolved = TableState::new([
            TableRow::new("row-ready")
                .with_cell("status", "Ready")
                .with_cell("score", 20_usize)
                .with_cell("enabled", true),
            TableRow::new("row-review")
                .with_cell("status", "Review")
                .with_cell("score", 30_usize)
                .with_cell("enabled", false),
            TableRow::new("row-blocked")
                .with_cell("status", "Blocked")
                .with_cell("score", 40_usize)
                .with_cell("enabled", true),
        ])
        .with_columns([
            TableColumn::new("status", "Status"),
            TableColumn::new("score", "Score"),
            TableColumn::new("enabled", "Enabled"),
        ])
        .with_filters([
            TableFilter::one_of("status", ["Ready", "Blocked"]),
            TableFilter::exact("enabled", "true"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-ready", "row-blocked"],
            "categorical filters use exact facet tokens and compose with other filters"
        );
    }

    #[test]
    fn text_predicate_filters_match_contains_equals_and_boundaries() {
        let state = TableState::new([
            TableRow::new("row-alpha")
                .with_cell("name", "Alpha Release")
                .with_cell("team", "UI"),
            TableRow::new("row-beta")
                .with_cell("name", "beta release")
                .with_cell("team", "Platform"),
            TableRow::new("row-gamma")
                .with_cell("name", "Gamma")
                .with_cell("team", "UI"),
        ])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
        ]);

        let contains = state
            .clone()
            .with_filters([TableFilter::contains("name", "release")]);
        assert_eq!(
            contains
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-alpha", "row-beta"],
            "contains should keep rows whose text includes the query"
        );

        let starts_with = state
            .clone()
            .with_filters([TableFilter::starts_with("name", "Al")]);
        assert_eq!(
            starts_with
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-alpha"],
            "starts_with should match the leading text"
        );

        let ends_with = state
            .clone()
            .with_filters([TableFilter::ends_with("name", "lease")]);
        assert_eq!(
            ends_with
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-alpha", "row-beta"],
            "ends_with should match trailing text"
        );

        let equals = state
            .clone()
            .with_filters([TableFilter::text_equals("name", "alpha release")]);
        assert_eq!(
            equals
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-alpha"],
            "case-insensitive equals should match exact text"
        );

        let case_sensitive = state.clone().with_filters([TableFilter::text_with_case(
            "name",
            TableTextFilterOperator::Equals,
            "Alpha Release",
            true,
        )]);
        assert_eq!(
            case_sensitive
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-alpha"],
            "case-sensitive equals should respect the original case"
        );

        let not_contains = state
            .clone()
            .with_filters([TableFilter::not_contains("name", "alpha")]);
        assert_eq!(
            not_contains
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-beta", "row-gamma"],
            "not_contains should remove matching rows"
        );

        assert_eq!(
            state
                .clone()
                .with_filters([TableFilter::text_with_case(
                    "name",
                    TableTextFilterOperator::NotEquals,
                    "Alpha Release",
                    true,
                )])
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-beta", "row-gamma"],
            "not_equals should exclude the exact match"
        );
    }

    #[test]
    fn numeric_comparison_filters_match_single_bounds_and_reject_invalid_bounds() {
        let state = TableState::new([
            TableRow::new("row-low").with_cell("score", 10_usize),
            TableRow::new("row-mid").with_cell("score", 20_usize),
            TableRow::new("row-high").with_cell("score", 30_usize),
            TableRow::new("row-text").with_cell("score", "30"),
            TableRow::new("row-missing").with_cell("team", "UI"),
        ])
        .with_columns([TableColumn::new("score", "Score")]);

        let greater = state
            .clone()
            .with_filters([TableFilter::number_greater_than("score", 10.0).expect("finite bound")]);
        assert_eq!(
            greater
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-mid", "row-high"],
            "greater-than should exclude the threshold value"
        );

        let greater_or_equal = state.clone().with_filters([
            TableFilter::number_greater_than_or_equal("score", 20.0).expect("finite bound"),
        ]);
        assert_eq!(
            greater_or_equal
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-mid", "row-high"],
            "greater-than-or-equal should keep the threshold value"
        );

        let less = state
            .clone()
            .with_filters([TableFilter::number_less_than("score", 30.0).expect("finite bound")]);
        assert_eq!(
            less.resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-low", "row-mid"],
            "less-than should exclude the threshold value"
        );

        let less_or_equal =
            state.clone().with_filters([
                TableFilter::number_less_than_or_equal("score", 20.0).expect("finite bound")
            ]);
        assert_eq!(
            less_or_equal
                .resolve()
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-low", "row-mid"],
            "less-than-or-equal should keep the threshold value"
        );

        assert!(
            TableFilter::number_greater_than("score", f64::NAN).is_none(),
            "invalid numeric bounds should not create a filter"
        );
        assert!(
            TableFilter::number_comparison(
                "score",
                TableNumericFilterOperator::LessThan,
                f64::INFINITY,
            )
            .is_none(),
            "infinite numeric bounds should not create a filter"
        );
    }

    #[test]
    fn manual_filtering_preserves_snapshot_and_global_filter_state() {
        let state = TableState::new([
            TableRow::new("row-1").with_cell("name", "Alpha"),
            TableRow::new("row-2").with_cell("name", "Beta"),
        ])
        .with_columns([TableColumn::new("name", "Name")])
        .with_global_filter("beta")
        .with_manual_filtering();
        let resolved = state.resolve();

        assert_eq!(state.global_filter(), Some("beta"));
        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1", "row-2"],
            "manual filtering should preserve the supplied row snapshot"
        );
        assert_eq!(resolved.global_facet_summary().row_count(), 2);
        assert_eq!(
            resolved
                .global_facet_summary()
                .column_facet(&TableColumnId::new("name"))
                .expect("global facet should resolve for the name column")
                .row_count(),
            2,
            "manual filtering should keep global facet summaries anchored to the supplied snapshot"
        );
    }

    #[test]
    fn categorical_filter_values_are_order_independent_cache_keys() {
        let left = TableFilter::one_of("status", ["Ready", "Blocked", "Ready"]);
        let right = TableFilter::one_of("status", ["Blocked", "Ready"]);

        assert_eq!(
            left, right,
            "selected categorical tokens are a deterministic set, not click order"
        );
        assert_eq!(
            left.selected_values()
                .expect("categorical filter should expose selected values")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            ["Blocked".to_string(), "Ready".to_string()]
        );

        let base = TableState::new(sample_rows()).with_columns([TableColumn::new("team", "Team")]);
        assert_eq!(
            base.clone().with_filters([left]).cache_key(),
            base.clone().with_filters([right]).cache_key(),
            "cache keys should not depend on selection order"
        );
        assert_ne!(
            base.clone()
                .with_filters([TableFilter::one_of("team", ["ops"])])
                .cache_key(),
            base.with_filters([TableFilter::one_of("team", ["design"])])
                .cache_key(),
            "changing the selected categorical token should invalidate caches"
        );
    }

    #[test]
    fn empty_categorical_filters_are_noops() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::one_of("team", std::iter::empty::<&str>())])
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-a", "row-c"],
            "an empty categorical filter should behave like no filter"
        );
    }

    #[test]
    fn numeric_range_filters_match_finite_number_cells_inclusively() {
        let resolved = TableState::new([
            TableRow::new("row-low").with_cell("score", 10_usize),
            TableRow::new("row-min").with_cell("score", 20_usize),
            TableRow::new("row-mid").with_cell("score", 25_usize),
            TableRow::new("row-max").with_cell("score", 30_usize),
            TableRow::new("row-high").with_cell("score", 40_usize),
            TableRow::new("row-text").with_cell("score", "30"),
            TableRow::new("row-missing").with_cell("team", "UI"),
            TableRow::new("row-infinite").with_cell("score", f64::INFINITY),
        ])
        .with_columns([TableColumn::new("score", "Score")])
        .with_filters([TableFilter::number_range("score", Some(20.0), Some(30.0))
            .expect("bounded numeric range should produce a filter")])
        .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-min", "row-mid", "row-max"],
            "range filters are inclusive and only match finite numeric cells"
        );
    }

    #[test]
    fn numeric_range_filters_normalize_open_and_reversed_bounds() {
        let min_only = TableFilter::number_range("score", Some(20.0), None)
            .expect("minimum-only range should produce a filter");
        assert_eq!(min_only.number_range_bounds(), Some((Some(20.0), None)));

        let max_only = TableFilter::number_range("score", Some(f64::NAN), Some(30.0))
            .expect("invalid minimum should become an open lower bound");
        assert_eq!(max_only.number_range_bounds(), Some((None, Some(30.0))));

        let reversed = TableFilter::number_range("score", Some(40.0), Some(10.0))
            .expect("reversed range should normalize");
        assert_eq!(
            reversed.number_range_bounds(),
            Some((Some(10.0), Some(40.0)))
        );

        assert!(
            TableFilter::number_range("score", Some(f64::NAN), Some(f64::INFINITY)).is_none(),
            "filters with no finite endpoints should be removable no-ops"
        );
    }

    #[test]
    fn pagination_total_page_count_uses_row_count_or_explicit_page_count() {
        let pagination = TablePagination::manual(2, 10, 42);

        assert_eq!(pagination.mode(), TableStageMode::Manual);
        assert!(pagination.is_manual());
        assert_eq!(pagination.page_index(), 2);
        assert_eq!(pagination.page_size(), 10);
        assert_eq!(pagination.row_count(), Some(42));
        assert_eq!(pagination.page_count(), Some(5));
        assert_eq!(pagination.with_page_count(9).page_count(), Some(9));
        assert_eq!(TablePagination::new(0, 10).page_count(), None);
        assert_eq!(TablePagination::manual(0, 0, 42).page_count(), Some(0));
    }

    #[test]
    fn manual_row_model_modes_preserve_supplied_snapshot() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .with_sorting([TableSort::ascending("score")])
            .with_manual_sorting()
            .with_pagination(TablePagination::manual(2, 1, 30))
            .resolve();

        let expected = ["row-b", "row-a", "row-c"];
        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn manual_stage_modes_participate_in_cache_keys() {
        let state = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::new(0, 1));

        assert_ne!(
            state.cache_key(),
            state.clone().with_manual_filtering().cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state.clone().with_manual_sorting().cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state
                .with_pagination(TablePagination::manual(0, 1, 30))
                .cache_key()
        );
    }

    #[test]
    fn row_pinning_state_dedupes_and_moves_rows_between_regions() {
        let pinning = TableRowPinning::new()
            .pinned_top(["row-a", "row-b", "row-a"])
            .pinned_bottom(["row-b", "row-c", "row-c"]);

        assert_eq!(
            pinning
                .top()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-a"],
            "bottom pins remove overlapping top pins"
        );
        assert_eq!(
            pinning
                .bottom()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-c"]
        );

        let moved = pinning.pinned_top(["row-c"]);
        assert_eq!(
            moved
                .top()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-c"]
        );
        assert_eq!(
            moved
                .bottom()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-b"]
        );
    }

    #[test]
    fn row_pinning_keep_pinned_rows_partitions_final_model_around_page() {
        let resolved = TableState::new(sample_rows())
            .with_pagination(TablePagination::new(1, 1))
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-b"])
                    .pinned_bottom(["row-c"]),
            )
            .resolve();

        assert_eq!(
            resolved.row_pinning_policy(),
            TableRowPinningPolicy::KeepPinnedRows
        );
        assert_eq!(row_ids(resolved.paginated_model().rows()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().top()), ["row-b"]);
        assert_eq!(row_ids(resolved.row_regions().center()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().bottom()), ["row-c"]);
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["row-b", "row-a", "row-c"]
        );
    }

    #[test]
    fn row_pinning_page_only_policy_ignores_rows_outside_page() {
        let resolved = TableState::new(sample_rows())
            .with_pagination(TablePagination::new(1, 1))
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-b"])
                    .pinned_bottom(["row-c"]),
            )
            .with_row_pinning_policy(TableRowPinningPolicy::PageOnly)
            .resolve();

        assert_eq!(
            resolved.row_pinning_policy(),
            TableRowPinningPolicy::PageOnly
        );
        assert!(resolved.row_regions().top().is_empty());
        assert_eq!(row_ids(resolved.row_regions().center()), ["row-a"]);
        assert!(resolved.row_regions().bottom().is_empty());
        assert_eq!(row_ids(resolved.final_model().rows()), ["row-a"]);
    }

    #[test]
    fn row_pinning_ignores_unknown_filtered_and_collapsed_rows() {
        let filtered = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["missing", "row-a"])
                    .pinned_bottom(["row-c"]),
            )
            .with_pagination(TablePagination::disabled())
            .resolve();

        assert!(filtered.row_regions().top().is_empty());
        assert_eq!(row_ids(filtered.row_regions().center()), ["row-b"]);
        assert_eq!(row_ids(filtered.row_regions().bottom()), ["row-c"]);
        assert_eq!(row_ids(filtered.final_model().rows()), ["row-b", "row-c"]);

        let collapsed = TableState::new(tree_rows())
            .with_columns([TableColumn::new("name", "Name")])
            .with_row_pinning(TableRowPinning::new().pinned_top(["pkg-core-test"]))
            .resolve();

        assert!(
            collapsed.row_regions().top().is_empty(),
            "collapsed descendants are not promoted into pinned bands"
        );
        assert_eq!(row_ids(collapsed.final_model().rows()), ["pkg", "docs"]);
        assert!(
            collapsed
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "collapsed descendants remain addressable through row lookup"
        );
    }

    #[test]
    fn row_pinning_preserves_duplicate_source_row_instances_in_visual_order() {
        let resolved = TableState::new([
            TableRow::new("duplicate").with_cell("name", "First"),
            TableRow::new("unique").with_cell("name", "Middle"),
            TableRow::new("duplicate").with_cell("name", "Second"),
        ])
        .with_row_pinning(TableRowPinning::new().pinned_top(["duplicate"]))
        .resolve();

        assert_eq!(
            row_ids(resolved.row_regions().top()),
            ["duplicate", "duplicate"]
        );
        assert_eq!(row_ids(resolved.row_regions().center()), ["unique"]);
        assert!(resolved.row_regions().bottom().is_empty());
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["duplicate", "duplicate", "unique"]
        );
    }

    #[test]
    fn overlapping_raw_row_pinning_state_resolves_without_duplicates() {
        let resolved = TableState::new(sample_rows())
            .with_row_pinning(TableRowPinning::from_raw(
                [TableRowId::new("row-a"), TableRowId::new("row-a")],
                [TableRowId::new("row-a"), TableRowId::new("row-c")],
            ))
            .resolve();

        assert_eq!(row_ids(resolved.row_regions().top()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().bottom()), ["row-c"]);
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["row-a", "row-b", "row-c"]
        );
    }

    #[test]
    fn selection_policy_single_keeps_only_one_selected_row() {
        let state = TableState::new(sample_rows())
            .with_selection_mode(TableSelectionMode::Single)
            .with_selected_rows(["row-a", "row-c"]);

        assert_eq!(
            state
                .selected_rows()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["row-a"]
        );
        assert_eq!(
            state.selection_policy().selection_mode(),
            TableSelectionMode::Single
        );
    }

    #[test]
    fn selection_policy_descendants_propagates_to_tree_children() {
        let resolved = TableState::new(tree_rows())
            .with_selection_policy(
                TableSelectionPolicy::default()
                    .with_sub_row_policy(TableSubRowSelectionPolicy::Descendants),
            )
            .with_all_rows_expanded()
            .with_selected_rows(["pkg"])
            .resolve();

        let selected_ids = resolved
            .core_model()
            .rows()
            .iter()
            .filter(|row| row.selected())
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>();

        assert_eq!(selected_ids, ["pkg", "pkg-ui", "pkg-core", "pkg-core-test"]);
        assert_eq!(resolved.core_selection_summary().selected_count(), 4);
        assert!(resolved.core_selection_summary().is_some_selected());
        assert_eq!(resolved.final_selection_summary().selected_count(), 4);
    }

    #[test]
    fn selection_summaries_report_all_some_and_none() {
        let all = TableState::new(sample_rows())
            .with_selected_rows(["row-a", "row-b", "row-c"])
            .resolve();
        let some = TableState::new(sample_rows())
            .with_selected_rows(["row-a"])
            .resolve();
        let none = TableState::new(sample_rows()).resolve();

        assert!(all.final_selection_summary().is_all_selected());
        assert_eq!(all.final_selection_summary().state().as_str(), "all");
        assert!(some.final_selection_summary().is_some_selected());
        assert_eq!(some.final_selection_summary().state().as_str(), "some");
        assert!(none.final_selection_summary().is_none_selected());
        assert_eq!(none.final_selection_summary().state().as_str(), "none");
    }

    #[test]
    fn full_and_current_page_selection_summaries_use_different_scopes() {
        let resolved = TableState::new(sample_rows())
            .with_selected_rows(["row-c"])
            .with_pagination(TablePagination::new(0, 1))
            .resolve();

        assert_eq!(resolved.full_selection_summary().selected_count(), 1);
        assert_eq!(
            resolved.current_page_selection_summary().selected_count(),
            0
        );
        assert!(resolved.full_selection_summary().is_some_selected());
        assert!(resolved.current_page_selection_summary().is_none_selected());
    }

    #[test]
    fn row_pinning_inputs_participate_in_cache_keys() {
        let state = TableState::new(sample_rows());

        assert_ne!(
            state.cache_key(),
            state
                .clone()
                .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
                .cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state
                .with_row_pinning_policy(TableRowPinningPolicy::PageOnly)
                .cache_key()
        );
    }

    #[test]
    fn facet_values_are_deterministic_and_ranges_ignore_non_numeric_values() {
        let resolved = TableState::new([
            TableRow::new("row-empty").with_cell("score", 4_usize),
            TableRow::new("row-bool")
                .with_cell("mixed", true)
                .with_cell("score", "n/a"),
            TableRow::new("row-number")
                .with_cell("mixed", 1_usize)
                .with_cell("score", 10_usize),
            TableRow::new("row-number-2")
                .with_cell("mixed", 1_usize)
                .with_cell("score", f64::INFINITY),
            TableRow::new("row-text")
                .with_cell("mixed", "1")
                .with_cell("score", f64::NAN),
        ])
        .with_columns([
            TableColumn::new("mixed", "Mixed"),
            TableColumn::new("score", "Score"),
        ])
        .resolve();

        let mixed = resolved
            .column_facet(&TableColumnId::new("mixed"))
            .expect("mixed facet should resolve");

        assert_eq!(mixed.mode(), TableStageMode::Client);
        assert_eq!(mixed.row_count(), 5);
        assert_eq!(mixed.unique_values().len(), 4);
        assert!(matches!(
            mixed.unique_values()[0].value(),
            TableCellValue::Empty
        ));
        assert_eq!(mixed.unique_values()[0].count(), 1);
        assert!(matches!(
            mixed.unique_values()[1].value(),
            TableCellValue::Bool(true)
        ));
        assert_eq!(mixed.unique_values()[1].count(), 1);
        assert!(matches!(
            mixed.unique_values()[2].value(),
            TableCellValue::Number(value) if *value == 1.0
        ));
        assert_eq!(mixed.unique_values()[2].count(), 2);
        assert!(matches!(
            mixed.unique_values()[3].value(),
            TableCellValue::Text(value) if value == "1"
        ));
        assert_eq!(mixed.unique_values()[3].count(), 1);

        let score = resolved
            .column_facet(&TableColumnId::new("score"))
            .expect("score facet should resolve");
        let range = score
            .numeric_range()
            .expect("finite score values should produce a range");
        assert_eq!(range.min(), 4.0);
        assert_eq!(range.max(), 10.0);
    }

    #[test]
    fn client_facets_exclude_own_filter_and_ignore_pagination() {
        let resolved = TableState::new([
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
        .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1"],
            "pagination still limits the final row model"
        );

        let status = resolved
            .column_facet(&TableColumnId::new("status"))
            .expect("status facet should resolve");
        assert_eq!(status.row_count(), 3);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 1), ("Ready".to_string(), 2)],
            "status facet ignores its own filter and honors the team filter"
        );

        let team = resolved
            .column_facet(&TableColumnId::new("team"))
            .expect("team facet should resolve");
        assert_eq!(team.row_count(), 3);
        assert_eq!(
            text_facet_counts(team),
            [("API".to_string(), 1), ("UI".to_string(), 2)],
            "team facet ignores its own filter and honors the status filter"
        );
    }

    #[test]
    fn richer_text_filters_compose_with_facets_and_global_query() {
        let resolved = TableState::new([
            TableRow::new("row-1")
                .with_cell("team", "UI")
                .with_cell("status", "Ready")
                .with_cell("name", "Done Alpha"),
            TableRow::new("row-2")
                .with_cell("team", "UI")
                .with_cell("status", "Blocked")
                .with_cell("name", "Done Beta"),
            TableRow::new("row-3")
                .with_cell("team", "API")
                .with_cell("status", "Ready")
                .with_cell("name", "Done Gamma"),
            TableRow::new("row-4")
                .with_cell("team", "UX")
                .with_cell("status", "Review")
                .with_cell("name", "Later"),
        ])
        .with_columns([
            TableColumn::new("team", "Team"),
            TableColumn::new("status", "Status"),
            TableColumn::new("name", "Name"),
        ])
        .with_filters([
            TableFilter::starts_with("team", "u"),
            TableFilter::text_with_case(
                "status",
                TableTextFilterOperator::NotEquals,
                "Blocked",
                true,
            ),
        ])
        .with_global_filter("done")
        .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1"],
            "rich text predicates should compose with the global filter"
        );

        let status = resolved
            .column_facet(&TableColumnId::new("status"))
            .expect("status facet should resolve");
        assert_eq!(status.row_count(), 2);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 1), ("Ready".to_string(), 1)],
            "status facets should ignore their own richer predicate while honoring other filters"
        );

        let team = resolved
            .column_facet(&TableColumnId::new("team"))
            .expect("team facet should resolve");
        assert_eq!(team.row_count(), 2);
        assert_eq!(
            text_facet_counts(team),
            [("API".to_string(), 1), ("UI".to_string(), 1)],
            "team facets should ignore their own richer predicate while honoring the status filter"
        );
    }

    #[test]
    fn richer_text_predicates_participate_in_cache_keys() {
        let base = TableState::new(sample_rows()).with_columns([TableColumn::new("team", "Team")]);
        let contains = base.clone().with_filters([TableFilter::text(
            "team",
            TableTextFilterOperator::Contains,
            "ops",
        )]);
        let starts = base.clone().with_filters([TableFilter::text(
            "team",
            TableTextFilterOperator::StartsWith,
            "ops",
        )]);
        let case_sensitive = base.clone().with_filters([TableFilter::text_with_case(
            "team",
            TableTextFilterOperator::Contains,
            "ops",
            true,
        )]);

        assert_ne!(
            contains.cache_key(),
            starts.cache_key(),
            "different text operators should invalidate caches"
        );
        assert_ne!(
            contains.cache_key(),
            case_sensitive.cache_key(),
            "case-sensitivity should participate in cache keys"
        );
    }

    #[test]
    fn global_facet_summary_honors_column_filters_and_excludes_global_query() {
        let resolved = TableState::new([
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
        .resolve();

        assert!(
            resolved.filtered_model().rows().is_empty(),
            "global query should not match text from columns opted out of global filtering"
        );
        assert_eq!(
            resolved
                .column_facet(&TableColumnId::new("status"))
                .expect("status column facet should resolve")
                .row_count(),
            0,
            "column facets should honor the active global query"
        );

        let summary = resolved.global_facet_summary();
        assert_eq!(summary.mode(), TableStageMode::Client);
        assert_eq!(summary.row_count(), 2);
        assert_eq!(
            summary
                .column_facets()
                .iter()
                .map(|facet| facet.column().as_str())
                .collect::<Vec<_>>(),
            ["team", "status", "score", "enabled", "tag"],
            "global facets should only include globally filterable columns"
        );
        assert!(summary.column_facet(&TableColumnId::new("notes")).is_none());

        let status = summary
            .column_facet(&TableColumnId::new("status"))
            .expect("status global facet should resolve");
        assert_eq!(status.row_count(), 2);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 1), ("Ready".to_string(), 1)]
        );

        let score = summary
            .column_facet(&TableColumnId::new("score"))
            .expect("score global facet should resolve");
        let score_range = score
            .numeric_range()
            .expect("score global facet should expose a numeric range");
        assert_eq!(score_range.min(), 10.0);
        assert_eq!(score_range.max(), 20.0);

        let enabled = summary
            .column_facet(&TableColumnId::new("enabled"))
            .expect("enabled global facet should resolve");
        assert!(matches!(
            enabled.unique_values()[0].value(),
            TableCellValue::Bool(false)
        ));
        assert_eq!(enabled.unique_values()[0].count(), 1);
        assert!(matches!(
            enabled.unique_values()[1].value(),
            TableCellValue::Bool(true)
        ));
        assert_eq!(enabled.unique_values()[1].count(), 1);

        let tag = summary
            .column_facet(&TableColumnId::new("tag"))
            .expect("tag global facet should resolve");
        assert!(matches!(
            tag.unique_values()[0].value(),
            TableCellValue::Empty
        ));
        assert_eq!(tag.unique_values()[0].count(), 1);
        assert!(matches!(
            tag.unique_values()[1].value(),
            TableCellValue::Text(value) if value == "alpha"
        ));
        assert_eq!(tag.unique_values()[1].count(), 1);
    }

    #[test]
    fn manual_filtering_client_facets_describe_supplied_snapshot() {
        let resolved = TableState::new(sample_rows())
            .with_columns([TableColumn::new("team", "Team")])
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .resolve();

        let team = resolved
            .column_facet(&TableColumnId::new("team"))
            .expect("team facet should resolve");

        assert_eq!(team.mode(), TableStageMode::Client);
        assert_eq!(team.row_count(), 3);
        assert_eq!(
            text_facet_counts(team),
            [("design".to_string(), 1), ("ops".to_string(), 2)],
            "manual filtering leaves client facets scoped to the supplied snapshot"
        );
    }

    #[test]
    fn manual_facet_payloads_override_client_facets_and_cache_keys() {
        let base = TableState::new([
            TableRow::new("row-1").with_cell("status", "Ready"),
            TableRow::new("row-2").with_cell("status", "Ready"),
        ])
        .with_columns([TableColumn::new("status", "Status")]);
        let server_facets = TableColumnFacets::manual("status", 64).with_unique_values([
            TableFacetValueCount::new("Blocked", 24),
            TableFacetValueCount::new("Ready", 40),
        ]);

        let resolved = base
            .clone()
            .with_manual_facets([server_facets.clone()])
            .resolve();
        let status = resolved
            .column_facet(&TableColumnId::new("status"))
            .expect("status facet should resolve");

        assert_eq!(status.mode(), TableStageMode::Manual);
        assert_eq!(status.row_count(), 64);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 24), ("Ready".to_string(), 40)],
            "manual payloads should not be derived from the current row snapshot"
        );

        assert_ne!(
            base.cache_key(),
            base.clone().with_manual_faceting().cache_key(),
            "faceting ownership participates in cache keys"
        );
        assert_ne!(
            base.clone().with_manual_facets([server_facets]).cache_key(),
            base.clone()
                .with_manual_facets([TableColumnFacets::manual("status", 64)
                    .with_unique_values([TableFacetValueCount::new("Ready", 64)])])
                .cache_key(),
            "manual facet payload content participates in cache keys"
        );

        let nan_facets = TableColumnFacets::manual("status", 2)
            .with_unique_values([TableFacetValueCount::new(f64::NAN, 2)]);
        let same_nan_facets = TableColumnFacets::manual("status", 2)
            .with_unique_values([TableFacetValueCount::new(f64::NAN, 2)]);
        assert_eq!(
            nan_facets, same_nan_facets,
            "facet equality should use stable numeric keys instead of raw f64 equality"
        );
        assert_eq!(
            base.clone().with_manual_facets([nan_facets]).cache_key(),
            base.clone()
                .with_manual_facets([same_nan_facets])
                .cache_key(),
            "manual facet NaN payloads should not make cache keys non-reflexive"
        );

        let unknown = base
            .with_manual_facets([TableColumnFacets::manual("missing", 10)])
            .resolve();
        assert!(
            unknown
                .column_facet(&TableColumnId::new("missing"))
                .is_none()
        );
        assert!(
            unknown
                .column_facet(&TableColumnId::new("status"))
                .is_some(),
            "unknown manual payloads do not corrupt configured-column facets"
        );
    }

    #[test]
    fn row_lookup_does_not_depend_on_numeric_index_positions() {
        let resolved = TableState::new(sample_rows())
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let row_c = resolved
            .core_model()
            .row(&TableRowId::new("row-c"))
            .expect("row-c should remain addressable by id");

        assert_eq!(row_c.source_index(), Some(2));
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-a", "row-b", "row-c"]
        );
    }

    #[test]
    fn selection_follows_row_ids_after_filtering_and_sorting() {
        let resolved = TableState::new(sample_rows())
            .with_selected_rows(["row-c"])
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let selected = resolved
            .final_model()
            .row(&TableRowId::new("row-c"))
            .expect("selected row should still be present");

        assert!(selected.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn nested_source_rows_resolve_parent_depth_and_lookup_metadata() {
        let resolved = TableState::new(tree_rows()).resolve();

        assert_eq!(
            resolved
                .core_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );

        let pkg = resolved
            .core_model()
            .row(&TableRowId::new("pkg"))
            .expect("root source row should be addressable");
        let pkg_tree = pkg.tree().expect("source row should expose tree metadata");
        assert_eq!(pkg.source_index(), Some(0));
        assert_eq!(pkg.depth(), 0);
        assert_eq!(pkg.parent_id(), None);
        assert!(pkg.is_tree_branch());
        assert_eq!(pkg.tree_expanded(), Some(false));
        assert_eq!(pkg_tree.descendant_count(), 3);

        let nested = resolved
            .core_model()
            .row(&TableRowId::new("pkg-core-test"))
            .expect("nested descendant should be addressable");
        assert_eq!(nested.source_index(), Some(3));
        assert_eq!(nested.depth(), 2);
        assert_eq!(nested.parent_id().map(TableRowId::as_str), Some("pkg-core"));
        assert!(!nested.is_tree_branch());
        assert_eq!(nested.descendant_count(), 0);
    }

    #[test]
    fn collapsed_tree_rows_hide_descendants_but_preserve_lookup() {
        let resolved = TableState::new(tree_rows()).resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "docs"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "collapsed tree descendants should remain addressable by stable row id"
        );
    }

    #[test]
    fn expanded_tree_rows_show_descendants_with_parent_depth_and_selection() {
        let resolved = TableState::new(tree_rows())
            .with_expanded_rows(["pkg", "pkg-core"])
            .with_selected_rows(["pkg-core-test"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );

        let pkg_core = resolved
            .final_model()
            .row(&TableRowId::new("pkg-core"))
            .expect("expanded branch should be addressable");
        assert_eq!(pkg_core.tree_expanded(), Some(true));
        assert_eq!(pkg_core.depth(), 1);
        assert_eq!(pkg_core.parent_id().map(TableRowId::as_str), Some("pkg"));

        let nested = resolved
            .final_model()
            .rows()
            .iter()
            .find(|row| row.id().as_str() == "pkg-core-test")
            .expect("expanded nested descendant should be visible");
        assert!(nested.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn child_expansion_does_not_bypass_collapsed_parent() {
        let resolved = TableState::new(tree_rows())
            .with_expanded_rows(["pkg-core"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "docs"]
        );
    }

    #[test]
    fn all_rows_expanded_expands_source_tree_branches() {
        let resolved = TableState::new(tree_rows())
            .with_all_rows_expanded()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(true)
        );
    }

    #[test]
    fn expandable_unloaded_source_rows_resolve_as_tree_branches() {
        let resolved = TableState::new([TableRow::new("remote-root")
            .with_cell("team", "remote")
            .with_expandable(true)])
        .resolve();

        let remote = resolved
            .final_model()
            .row(&TableRowId::new("remote-root"))
            .expect("expandable source row should resolve");
        let tree = remote
            .tree()
            .expect("expandable source row should expose tree metadata");

        assert!(remote.is_tree_branch());
        assert_eq!(remote.tree_expanded(), Some(false));
        assert!(!tree.has_children());
        assert!(tree.can_expand());
        assert_eq!(tree.loaded_child_count(), 0);
        assert_eq!(tree.children_load_state(), &TableRowChildrenLoadState::Idle);
        assert_eq!(remote.loaded_child_count(), 0);
        assert_eq!(
            remote.children_load_state(),
            Some(&TableRowChildrenLoadState::Idle)
        );
    }

    #[test]
    fn child_loading_metadata_survives_row_lookup() {
        let resolved = TableState::new([
            TableRow::new("loading").with_children_loading("Loading packages"),
            TableRow::new("failed").with_children_load_failed("Network unavailable"),
        ])
        .resolve();

        let loading = resolved
            .final_model()
            .row(&TableRowId::new("loading"))
            .expect("loading branch should resolve");
        let failed = resolved
            .final_model()
            .row(&TableRowId::new("failed"))
            .expect("failed branch should resolve");

        assert!(loading.is_tree_branch());
        assert_eq!(
            loading
                .children_load_state()
                .and_then(|state| state.message()),
            Some("Loading packages")
        );
        assert!(
            loading
                .children_load_state()
                .is_some_and(TableRowChildrenLoadState::is_loading)
        );
        assert!(failed.is_tree_branch());
        assert_eq!(
            failed
                .children_load_state()
                .and_then(|state| state.message()),
            Some("Network unavailable")
        );
        assert!(
            failed
                .children_load_state()
                .is_some_and(TableRowChildrenLoadState::is_failed)
        );
    }

    #[test]
    fn manual_expansion_keeps_supplied_tree_descendants_visible() {
        let resolved = TableState::new(tree_rows())
            .with_manual_expansion()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(false)
        );
    }

    #[test]
    fn manual_expansion_preserves_expanded_metadata_without_pruning() {
        let resolved = TableState::new(tree_rows())
            .with_manual_expansion()
            .with_expanded_rows(["pkg"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(true)
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(false)
        );
    }

    #[test]
    fn manual_expansion_does_not_bypass_grouped_row_expansion() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_manual_expansion()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "group:team=design"]
        );
    }

    #[test]
    fn tree_filtering_uses_parent_to_child_policy() {
        let resolved = TableState::new(tree_rows())
            .with_filters([TableFilter::contains("team", "core")])
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-core", "pkg-core-test"]
        );

        let leaf_match_without_parent = TableState::new(tree_rows())
            .with_filters([TableFilter::contains("team", "ui")])
            .resolve();
        assert!(
            leaf_match_without_parent.filtered_model().rows().is_empty(),
            "first slice keeps TanStack's default parent-to-child filtering policy"
        );
    }

    #[test]
    fn pagination_applies_after_tree_expansion() {
        let resolved = TableState::new(tree_rows())
            .with_all_rows_expanded()
            .with_pagination(TablePagination::new(0, 3))
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "expanded-but-not-paginated tree descendants should remain addressable"
        );
    }

    #[test]
    fn duplicate_row_ids_are_reported_across_nested_source_rows() {
        let resolved = TableState::new([
            TableRow::new("root").with_child(TableRow::new("duplicate")),
            TableRow::new("duplicate"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .duplicate_row_ids()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["duplicate"]
        );
    }

    #[test]
    fn cache_key_row_count_includes_child_topology() {
        let flat = TableState::new([TableRow::new("root")]);
        let nested = TableState::new([TableRow::new("root").with_child(TableRow::new("child"))]);

        assert_eq!(flat.cache_key().row_count(), 1);
        assert_eq!(nested.cache_key().row_count(), 2);
        assert_ne!(flat.cache_key(), nested.cache_key());
    }

    #[test]
    fn grouping_keeps_source_tree_rows_out_of_the_grouped_path() {
        let resolved = TableState::new(tree_rows())
            .with_grouping(["team"])
            .with_all_rows_expanded()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=core", "pkg", "group:team=docs", "docs"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-ui"))
                .is_none(),
            "tree plus grouping composition is deferred for a later policy slice"
        );
    }

    #[test]
    fn grouped_row_model_creates_stable_group_rows() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .grouped_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            [
                "group:team=ops",
                "row-b",
                "row-c",
                "group:team=design",
                "row-a"
            ]
        );

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group row should be addressable by id");
        let ops_group = ops.group().expect("ops row should be a group row");

        assert_eq!(ops_group.grouping_column().as_str(), "team");
        assert_eq!(ops_group.grouping_value().filter_text(), "ops");
        assert_eq!(ops_group.depth(), 0);
        assert_eq!(ops_group.parent_id(), None);
        assert_eq!(ops_group.first_leaf_row_id().as_str(), "row-b");
        assert_eq!(ops_group.leaf_row_count(), 2);
        assert!(ops.is_group());
    }

    #[test]
    fn collapsed_groups_hide_descendants_but_preserve_lookup() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .expanded_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "group:team=design"]
        );
        assert!(
            resolved
                .expanded_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "collapsed descendants should remain addressable in lookup metadata"
        );
    }

    #[test]
    fn expanded_groups_show_descendants_with_parent_depth_and_selection() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_expanded_rows(["group:team=ops"])
            .with_selected_rows(["row-c"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b", "row-c", "group:team=design"]
        );

        let row_c = resolved
            .final_model()
            .rows()
            .iter()
            .find(|row| row.id().as_str() == "row-c")
            .expect("expanded descendant should be visible");

        assert_eq!(row_c.depth(), 1);
        assert_eq!(
            row_c.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert!(row_c.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn multi_column_grouping_creates_nested_group_paths() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team", "score"])
            .resolve();

        let nested = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops>score=20"))
            .expect("nested score group should use the parent path");
        let group = nested.group().expect("nested row should be grouped");

        assert_eq!(group.depth(), 1);
        assert_eq!(
            group.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert_eq!(group.leaf_row_count(), 1);
    }

    #[test]
    fn pagination_applies_after_expansion() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_all_rows_expanded()
            .with_pagination(TablePagination::new(0, 2))
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "final lookup keeps expanded-but-not-paginated rows addressable"
        );
    }

    #[test]
    fn aggregate_kind_labels_are_stable() {
        assert_eq!(TableAggregateKind::Count.as_str(), "count");
        assert_eq!(TableAggregateKind::Sum.as_str(), "sum");
        assert_eq!(TableAggregateKind::Min.as_str(), "min");
        assert_eq!(TableAggregateKind::Max.as_str(), "max");
        assert_eq!(TableAggregateKind::Average.as_str(), "average");
    }

    #[test]
    fn grouped_rows_expose_builtin_aggregate_cells() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::sum("score"),
                TableAggregation::min("low"),
                TableAggregation::max("high"),
                TableAggregation::average("duration"),
                TableAggregation::sum("noise"),
            ])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("name")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("score")),
            Some(&TableCellValue::Number(50.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("low")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("high")),
            Some(&TableCellValue::Number(11.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("duration")),
            Some(&TableCellValue::Number(3.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("noise")),
            Some(&TableCellValue::Empty)
        );
    }

    #[test]
    fn grouped_rows_resolve_named_custom_aggregation_callbacks() {
        let state = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::named("score", "score_plus_one"),
                TableAggregation::named("duration", "sum"),
                TableAggregation::named("noise", "missing_custom"),
            ])
            .with_aggregation_fn("score_plus_one", |column, rows| {
                TableCellValue::Number(
                    numeric_values(rows, column).fold(0.0, |sum, value| sum + value) + 1.0,
                )
            });

        let resolved = state.resolve();
        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("score")),
            Some(&TableCellValue::Number(51.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("duration")),
            Some(&TableCellValue::Number(6.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("noise")),
            Some(&TableCellValue::Empty)
        );
        assert_eq!(state.aggregation_fn_count(), 1);
        assert!(state.has_aggregation_fn("score_plus_one"));
        assert!(!state.has_aggregation_fn("missing_custom"));
        assert_ne!(
            state.cache_key(),
            state
                .clone()
                .with_aggregation_fn("score_plus_one", |column, rows| {
                    TableCellValue::Number(
                        numeric_values(rows, column).fold(0.0, |sum, value| sum + value) + 2.0,
                    )
                })
                .cache_key()
        );
    }

    #[test]
    fn grouping_value_overrides_aggregate_on_grouping_column() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([TableAggregation::count("team")])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("team")),
            Some(&TableCellValue::Text("ops".to_string()))
        );
    }

    #[test]
    fn visible_columns_respect_explicit_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .with_column_order(["score", "team", "name"])
            .with_column_visibility(
                TableColumnVisibilityOverrides::new()
                    .show("team")
                    .hide("score"),
            )
            .resolve();

        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["team", "name"]
        );
    }

    #[test]
    fn flat_table_resolves_a_single_header_row_per_region() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team"),
                TableColumn::new("status", "Status"),
            ])
            .resolve();

        assert!(resolved.header_groups().left().is_empty());
        assert_eq!(resolved.header_groups().center().len(), 1);
        assert!(resolved.header_groups().right().is_empty());
        assert_eq!(resolved.center_header_groups()[0].depth(), 0);
        assert_eq!(resolved.center_header_groups()[0].headers().len(), 3);
        assert!(
            resolved.center_header_groups()[0]
                .headers()
                .iter()
                .all(|header| header.is_leaf())
        );
        assert_eq!(
            resolved.center_header_groups()[0]
                .headers()
                .iter()
                .map(|header| header.id().to_owned())
                .collect::<Vec<_>>(),
            ["name", "team", "status"]
        );
    }

    #[test]
    fn nested_groups_resolve_group_rows_and_placeholders() {
        let resolved = TableState::new(sample_rows())
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
                            TableColumn::new("score", "Score"),
                            TableColumn::new("status", "Status"),
                        ],
                    )],
                ),
            ])
            .resolve();

        let center_groups = resolved.center_header_groups();
        assert_eq!(center_groups.len(), 3);
        assert_eq!(center_groups[0].depth(), 0);
        assert_eq!(center_groups[1].depth(), 1);
        assert_eq!(center_groups[2].depth(), 2);
        assert_eq!(
            center_groups[0]
                .headers()
                .iter()
                .map(|header| (header.label().to_owned(), header.col_span(), header.kind()))
                .collect::<Vec<_>>(),
            [
                ("Identity".to_string(), 2, TableResolvedHeaderKind::Group),
                ("Metrics".to_string(), 2, TableResolvedHeaderKind::Group),
            ]
        );
        assert_eq!(
            center_groups[1]
                .headers()
                .iter()
                .map(|header| header.label().to_owned())
                .collect::<Vec<_>>(),
            ["Name", "Team", "Scores"]
        );
        assert!(
            center_groups[2]
                .headers()
                .iter()
                .take(2)
                .all(|header| header.is_placeholder())
        );
        assert_eq!(
            center_groups[2]
                .headers()
                .iter()
                .skip(2)
                .map(|header| header.label().to_owned())
                .collect::<Vec<_>>(),
            ["Score", "Status"]
        );
    }

    #[test]
    fn hidden_leaves_shrink_header_spans_without_empty_groups() {
        let resolved = TableState::new(sample_rows())
            .with_column_tree([
                TableColumnGroup::new(
                    "identity",
                    "Identity",
                    [
                        TableColumn::new("name", "Name"),
                        TableColumn::new("team", "Team").with_visible(false),
                    ],
                ),
                TableColumnGroup::new(
                    "metrics",
                    "Metrics",
                    [TableColumnGroup::new(
                        "scores",
                        "Scores",
                        [
                            TableColumn::new("score", "Score"),
                            TableColumn::new("status", "Status").with_visible(false),
                        ],
                    )],
                ),
            ])
            .resolve();

        let center_groups = resolved.center_header_groups();
        assert_eq!(center_groups.len(), 3);
        assert_eq!(center_groups[0].headers()[0].col_span(), 1);
        assert_eq!(center_groups[0].headers()[0].label(), "Identity");
        assert_eq!(center_groups[0].headers()[0].leaf_column_ids().len(), 1);
        assert_eq!(
            center_groups[1]
                .headers()
                .iter()
                .map(|header| header.label().to_owned())
                .collect::<Vec<_>>(),
            ["Name", "Scores"]
        );
        assert_eq!(center_groups[2].headers()[0].col_span(), 1);
        assert!(center_groups[2].headers()[0].is_placeholder());
        assert_eq!(center_groups[2].headers()[1].label(), "Score");
    }

    #[test]
    fn pinned_regions_resolve_independent_header_families() {
        let resolved = TableState::new(sample_rows())
            .with_column_tree([
                TableColumnGroup::new(
                    "identity",
                    "Identity",
                    [
                        TableColumn::new("name", "Name"),
                        TableColumn::new("team", "Team"),
                    ],
                ),
                TableColumnGroup::new("metrics", "Metrics", [TableColumn::new("score", "Score")]),
            ])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["score"]),
            )
            .resolve();

        assert_eq!(resolved.left_header_groups().len(), 2);
        assert_eq!(
            resolved.left_header_groups()[0].headers()[0].label(),
            "Identity"
        );
        assert_eq!(resolved.left_header_groups()[1].headers()[0].id(), "name");
        assert_eq!(resolved.right_header_groups().len(), 2);
        assert_eq!(
            resolved.right_header_groups()[0].headers()[0].label(),
            "Metrics"
        );
        assert_eq!(resolved.right_header_groups()[1].headers()[0].id(), "score");
    }

    #[test]
    fn column_visibility_overrides_descriptor_defaults_and_preserves_other_state() {
        let base = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .with_column_order(["score", "team", "name"])
            .with_sorting([TableSort::descending("score")])
            .with_filters([TableFilter::contains("team", "ops")])
            .with_pagination(TablePagination::new(1, 1))
            .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(220.0)));
        let state = base.clone().with_column_visibility(
            TableColumnVisibilityOverrides::new()
                .show("team")
                .hide("score")
                .with_visibility("missing", true),
        );

        assert_eq!(state.column_visibility().len(), 3);
        assert_eq!(
            state
                .column_visibility()
                .override_for(&TableColumnId::new("score")),
            Some(false)
        );
        assert_eq!(
            state
                .column_visibility()
                .override_for(&TableColumnId::new("team")),
            Some(true)
        );
        assert_eq!(
            state
                .column_visibility()
                .override_for(&TableColumnId::new("missing")),
            Some(true)
        );
        assert_eq!(
            state
                .resolve()
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "team"]
        );
        assert_eq!(state.sorting(), base.sorting());
        assert_eq!(state.filters(), base.filters());
        assert_eq!(state.pagination(), base.pagination());
        assert_eq!(state.column_pinning(), base.column_pinning());
        assert_eq!(state.column_sizing(), base.column_sizing());
        assert_ne!(state.cache_key(), base.cache_key());
    }

    #[test]
    fn non_hideable_columns_ignore_hidden_overrides() {
        let resolved = TableState::new(sample_rows())
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
            .resolve();

        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "team"]
        );
        assert!(
            !resolved
                .visible_columns()
                .iter()
                .any(|column| column.id().as_str() == "score")
        );
    }

    #[test]
    fn pinned_columns_split_visible_regions_after_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
                TableColumn::new("owner", "Owner"),
                TableColumn::new("status", "Status"),
            ])
            .with_column_order(["status", "score", "owner", "team", "name"])
            .with_column_visibility(
                TableColumnVisibilityOverrides::new()
                    .show("team")
                    .hide("score"),
            )
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score", "missing"])
                    .pinned_right(["status"]),
            )
            .resolve();
        let regions = resolved.visible_column_regions();

        assert_eq!(TableColumnRegion::Left.as_str(), "left");
        assert_eq!(TableColumnRegion::Center.as_str(), "center");
        assert_eq!(TableColumnRegion::Right.as_str(), "right");
        assert_eq!(
            regions
                .left()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name"],
            "pinned left columns preserve resolved visible order"
        );
        assert_eq!(
            regions
                .center()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["owner", "team"],
            "unknown and invisible pinned ids are ignored"
        );
        assert_eq!(
            regions
                .right()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["status"]
        );
        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name", "owner", "team", "status"]
        );
    }

    #[test]
    fn column_pinning_moves_columns_between_regions_without_duplicates() {
        let pinning = TableColumnPinning::new()
            .pinned_left(["name", "score", "name"])
            .pinned_right(["score", "status", "score"]);

        assert_eq!(
            pinning
                .left()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            pinning
                .right()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["score", "status"]
        );
        assert!(!pinning.is_empty());
    }

    #[test]
    fn duplicate_row_ids_are_reported_without_panicking() {
        let resolved = TableState::new([
            TableRow::new("row-a").with_cell("name", "A"),
            TableRow::new("row-a").with_cell("name", "A duplicate"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .duplicate_row_ids()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["row-a"]
        );
    }

    #[test]
    fn cache_key_reuses_row_identity_for_clones_and_invalidates_state_changes() {
        let base = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);
        let cloned = base.clone();
        let sorted = base.clone().with_sorting([TableSort::descending("score")]);
        let aggregated = base
            .clone()
            .with_aggregations([TableAggregation::sum("score")]);
        let pinned = base.clone().with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["score"]),
        );
        let sized = base
            .clone()
            .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(180.0)));
        let rebuilt = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);

        assert_eq!(base, cloned);
        assert_eq!(base.cache_key(), cloned.cache_key());
        assert_eq!(
            base.cache_key().rows_identity(),
            cloned.cache_key().rows_identity()
        );

        assert_ne!(base.cache_key(), sorted.cache_key());
        assert_ne!(base.cache_key(), aggregated.cache_key());
        assert_ne!(base.cache_key(), pinned.cache_key());
        assert_ne!(base.cache_key(), sized.cache_key());
        assert_eq!(base, rebuilt);
        assert_ne!(
            base.cache_key().rows_identity(),
            rebuilt.cache_key().rows_identity()
        );
        assert_ne!(base.cache_key(), rebuilt.cache_key());
    }
}
