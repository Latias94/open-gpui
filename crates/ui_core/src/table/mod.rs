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
pub use identity::{
    TableColumnGroupId, TableColumnId, TableGroupNumberIdentity, TableGroupRowIdentity,
    TableGroupRowSegment, TableGroupValueIdentity, TableHeaderIdentity, TableHeaderRowIdentity,
    TableResolvedHeaderIdentity, TableRowId, TableRowIdentity, TableRowIdentityDiagnostic,
    TableRowIdentityKey, TableRowInstanceId, TableRowOccurrenceIdentity,
    TableSourceInstanceIdentity, TableSourceRowIdentity,
};
pub use resolved::{
    TableGroupRow, TableResolvedRow, TableResolvedRowKind, TableResolvedState, TableRowModel,
    TableTreeRow,
};
pub use row_model::{
    TABLE_ROW_MODEL_PIPELINE, TABLE_ROW_MODEL_V0_PIPELINE, TableExpansionMode, TableExpansionState,
    TablePagination, TableRowModelStage, TableSourceRowLookup, TableStageMode,
};
pub use rows::{
    TableRow, TableRowChildrenLoadState, TableRowPinTarget, TableRowPinning, TableRowPinningPolicy,
    TableRowRegion, TableRowRegions,
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

/// Convenient imports for renderer-neutral table model work.
pub mod prelude {
    pub use super::{
        TABLE_DEFAULT_COLUMN_WIDTH, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH,
        TABLE_ROW_MODEL_PIPELINE, TABLE_ROW_MODEL_V0_PIPELINE, TableAggregateKind,
        TableAggregation, TableCellEditor, TableCellValue, TableColumn, TableColumnFacets,
        TableColumnGroup, TableColumnGroupId, TableColumnId, TableColumnNode, TableColumnPinning,
        TableColumnRegion, TableColumnRegions, TableColumnResizeDirection, TableColumnResizeMode,
        TableColumnResizeState, TableColumnResizeUpdate, TableColumnSizing,
        TableColumnVisibilityOverrides, TableColumnWidthPolicy, TableExpansionMode,
        TableExpansionState, TableFacetRange, TableFacetValueCount, TableFilter, TableFilterKind,
        TableGlobalFacetSummary, TableGroupRow, TableGroupRowIdentity, TableGroupRowSegment,
        TableGroupValueIdentity, TableHeaderIdentity, TableHeaderRowIdentity,
        TableNumericFilterBound, TableNumericFilterOperator, TablePagination,
        TableResolvedColumnSizing, TableResolvedColumnSizingRegions, TableResolvedHeaderCell,
        TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions, TableResolvedHeaderIdentity,
        TableResolvedHeaderKind, TableResolvedRow, TableResolvedRowKind, TableResolvedState,
        TableRow, TableRowChildrenLoadState, TableRowId, TableRowIdentity,
        TableRowIdentityDiagnostic, TableRowIdentityKey, TableRowInstanceId, TableRowModel,
        TableRowModelStage, TableRowOccurrenceIdentity, TableRowPinTarget, TableRowPinning,
        TableRowPinningPolicy, TableRowRegion, TableRowRegions, TableSelectOption,
        TableSelectionActivationMode, TableSelectionMode, TableSelectionPolicy,
        TableSelectionSummary, TableSelectionSummaryState, TableSort, TableSortDirection,
        TableSourceInstanceIdentity, TableSourceRowIdentity, TableSourceRowLookup, TableStageMode,
        TableState, TableStateCacheKey, TableSubRowSelectionPolicy, TableTextFilterOperator,
        TableTreeRow, drag_table_column_resize, end_table_column_resize,
    };
}

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use aggregation::TableAggregationFn;
use columns::normalize_table_column_tree;
use faceting::{resolve_client_column_facets, resolve_client_global_column_facets};
use filtering::normalize_table_global_filter_query;
use row_model::{
    TableRowNode, TableSourceIdentityIndex, build_group_nodes, build_source_row_nodes,
    filter_source_row_nodes, filter_source_row_nodes_by_global_query, flatten_nodes,
    push_expanded_rows,
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
    source_identities: Arc<TableSourceIdentityIndex>,
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
        let rows_identity = next_table_rows_identity();
        let source_identities = Arc::new(TableSourceIdentityIndex::new(&rows, rows_identity));

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
            source_identities,
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
        let rows = rows.into_iter().collect::<Vec<_>>();
        let rows_identity = next_table_rows_identity();
        self.source_identities = Arc::new(TableSourceIdentityIndex::new(&rows, rows_identity));
        self.rows = rows.into();
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

    /// Applies exact expanded row identities.
    ///
    /// ```compile_fail
    /// use open_gpui_ui_core::{TableRow, TableState};
    ///
    /// let state = TableState::new(std::iter::empty::<TableRow>());
    /// let _ = state.with_expanded_rows(["row-a"]);
    /// ```
    pub fn with_expanded_rows(
        mut self,
        expanded_rows: impl IntoIterator<Item = TableRowIdentity>,
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

    /// Returns the complete canonical source-column order.
    ///
    /// Known explicit ids are emitted once, followed by every unlisted source column in source
    /// order. Unknown ids and duplicates are ignored. Visibility and pinning are independent
    /// projections and do not remove columns from this order.
    pub fn normalized_column_order(&self) -> Vec<TableColumnId> {
        let mut remaining = self
            .columns
            .iter()
            .map(|column| column.id().clone())
            .collect::<BTreeSet<_>>();
        let mut normalized = Vec::with_capacity(self.columns.len());

        for id in &self.column_order {
            if remaining.remove(id) {
                normalized.push(id.clone());
            }
        }
        for column in &self.columns {
            if remaining.remove(column.id()) {
                normalized.push(column.id().clone());
            }
        }

        normalized
    }

    /// Returns runtime column visibility overrides.
    pub const fn column_visibility(&self) -> &TableColumnVisibilityOverrides {
        &self.column_visibility
    }

    /// Returns source rows.
    pub fn rows(&self) -> &[TableRow] {
        self.rows.as_ref()
    }

    /// Resolves one source-row identity against the current caller-owned source snapshot.
    pub fn source_row_lookup(&self, identity: &TableSourceRowIdentity) -> TableSourceRowLookup {
        self.source_identities.lookup(identity)
    }

    /// Resolves the zero-based business-id occurrence to its exact current-snapshot identity.
    ///
    /// The returned identity is stable through row-model transformations and cloned table state,
    /// but becomes stale after [`Self::with_rows`]. Use caller-owned instance ids for retention
    /// across source replacement or reorder.
    pub fn source_row_identity_at(
        &self,
        row_id: impl Into<TableRowId>,
        occurrence: usize,
    ) -> Option<TableSourceRowIdentity> {
        let row_id = row_id.into();
        self.source_identities.identity_at(&row_id, occurrence)
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
            rows_identity: self.source_identities.source_snapshot(),
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
        let mut columns_by_id: BTreeMap<_, _> = self
            .columns
            .iter()
            .map(|column| (column.id().clone(), column.clone()))
            .collect();
        self.normalized_column_order()
            .into_iter()
            .filter_map(|id| columns_by_id.remove(&id))
            .filter(|column| self.column_visibility.is_visible(column))
            .collect()
    }

    /// Resolves row models from the input state.
    pub fn resolve(&self) -> TableResolvedState {
        let row_identity_diagnostics = self.source_identities.diagnostics();
        let include_source_children = self.grouping.is_empty();
        let global_filterable_columns = self.global_filterable_column_ids();
        let selected_rows = self
            .selection_policy
            .resolve_selected_rows(&self.rows, &self.selected_rows);
        let mut source_identity_cursor = self.source_identities.cursor();
        let source_nodes = build_source_row_nodes(
            &self.rows,
            &selected_rows,
            &self.expansion,
            include_source_children,
            &mut source_identity_cursor,
            None,
            0,
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
            &expanded_model,
            &paginated_model,
            &self.row_pinning,
            self.row_pinning_policy,
        );
        let final_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Final,
            row_regions.flattened(),
            expanded_model.lookup_rows().cloned(),
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
            row_identity_diagnostics,
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

        match (left.identity(), right.identity()) {
            (TableRowIdentity::Source(left), TableRowIdentity::Source(right)) => {
                left.row_id().cmp(right.row_id())
            }
            (TableRowIdentity::Group(left), TableRowIdentity::Group(right)) => left.cmp(right),
            _ => left.identity().cmp(right.identity()),
        }
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

#[cfg(test)]
mod tests;
