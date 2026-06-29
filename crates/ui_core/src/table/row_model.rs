//! Row-model stage, pagination, and expansion vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use super::aggregation::{TableAggregation, TableAggregationFn, resolve_aggregate_cells};
use super::faceting::row_matches_global_filter;
use super::filtering::TableFilter;
use super::resolved::{TableGroupRow, TableResolvedRow, TableTreeRow};
use super::rows::{TableRow, count_table_rows};
use super::{TableCellValue, TableColumnId, TableRowId};

/// Per-stage row-model ownership for client or manual control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStageMode {
    /// The table applies the stage locally.
    Client,
    /// The caller supplies the stage output snapshot.
    Manual,
}

impl TableStageMode {
    /// Returns whether the stage is caller-owned.
    pub const fn is_manual(self) -> bool {
        matches!(self, Self::Manual)
    }
}

impl Default for TableStageMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Pagination state for a table row model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablePagination {
    page_index: usize,
    page_size: usize,
    mode: TableStageMode,
    row_count: Option<usize>,
    page_count: Option<usize>,
}

impl TablePagination {
    /// Creates pagination state from a page index and page size.
    pub const fn new(page_index: usize, page_size: usize) -> Self {
        Self {
            page_index,
            page_size,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Creates manual pagination state from a page index, page size, and total row count.
    pub const fn manual(page_index: usize, page_size: usize, row_count: usize) -> Self {
        Self::new(page_index, page_size)
            .with_mode(TableStageMode::Manual)
            .with_row_count(row_count)
    }

    /// Returns pagination that keeps all rows.
    pub const fn disabled() -> Self {
        Self {
            page_index: 0,
            page_size: usize::MAX,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Returns the zero-based page index.
    pub const fn page_index(self) -> usize {
        self.page_index
    }

    /// Returns the same pagination state with the page index reset.
    pub const fn with_page_index(mut self, page_index: usize) -> Self {
        self.page_index = page_index;
        self
    }

    /// Returns the maximum number of rows per page.
    pub const fn page_size(self) -> usize {
        self.page_size
    }

    /// Returns the pagination ownership mode.
    pub const fn mode(self) -> TableStageMode {
        self.mode
    }

    /// Returns whether pagination is caller-owned.
    pub const fn is_manual(self) -> bool {
        self.mode.is_manual()
    }

    /// Returns the total row count when known.
    pub const fn row_count(self) -> Option<usize> {
        self.row_count
    }

    /// Returns the total page count when known or derivable.
    pub fn page_count(self) -> Option<usize> {
        if let Some(page_count) = self.page_count {
            return Some(page_count);
        }

        let row_count = self.row_count?;
        if self.page_size == 0 {
            return Some(0);
        }

        Some(row_count.div_ceil(self.page_size))
    }

    /// Applies pagination ownership mode.
    pub const fn with_mode(mut self, mode: TableStageMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies a total row count.
    pub const fn with_row_count(mut self, row_count: usize) -> Self {
        self.row_count = Some(row_count);
        self
    }

    /// Applies a total page count.
    pub const fn with_page_count(mut self, page_count: usize) -> Self {
        self.page_count = Some(page_count);
        self
    }

    pub(super) fn apply(self, rows: &[TableResolvedRow]) -> Vec<TableResolvedRow> {
        if self.is_manual() || self.page_size == usize::MAX {
            return rows.to_vec();
        }
        if self.page_size == 0 {
            return Vec::new();
        }

        let start = self.page_index.saturating_mul(self.page_size);
        rows.iter()
            .skip(start)
            .take(self.page_size)
            .cloned()
            .collect()
    }
}

impl Default for TablePagination {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Caller-owned expansion state for grouped table rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableExpansionState {
    /// Every group row is expanded.
    All,
    /// Only the listed stable row ids are expanded.
    Rows(BTreeSet<TableRowId>),
}

impl TableExpansionState {
    /// Returns an expansion state where every row is expanded.
    pub const fn all() -> Self {
        Self::All
    }

    /// Returns an expansion state for explicit row ids.
    pub fn rows(rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        Self::Rows(rows.into_iter().map(Into::into).collect())
    }

    /// Returns whether the given row id should be expanded.
    pub fn is_expanded(&self, row_id: &TableRowId) -> bool {
        match self {
            Self::All => true,
            Self::Rows(rows) => rows.contains(row_id),
        }
    }
}

impl Default for TableExpansionState {
    fn default() -> Self {
        Self::Rows(BTreeSet::new())
    }
}

/// Row expansion behavior for resolved table row models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableExpansionMode {
    /// The core row model hides descendants of collapsed rows.
    Client,
    /// The caller supplies the visible source-tree snapshot.
    Manual,
}

impl TableExpansionMode {
    /// Returns whether local row-model expansion pruning is enabled.
    pub const fn prunes_collapsed_rows(self) -> bool {
        matches!(self, Self::Client)
    }
}

impl Default for TableExpansionMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Row-model stage vocabulary for Open GPUI tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowModelStage {
    /// Materialized one-to-one data rows.
    Core,
    /// Filtered rows.
    Filtered,
    /// Grouped rows.
    Grouped,
    /// Sorted rows.
    Sorted,
    /// Expanded rows.
    Expanded,
    /// Paginated rows.
    Paginated,
    /// Final row model consumed by renderers.
    Final,
}

impl TableRowModelStage {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Filtered => "filtered",
            Self::Grouped => "grouped",
            Self::Sorted => "sorted",
            Self::Expanded => "expanded",
            Self::Paginated => "paginated",
            Self::Final => "final",
        }
    }

    /// Returns whether this stage belonged to the original v0 resolver subset.
    pub const fn implemented_in_v0(self) -> bool {
        matches!(
            self,
            Self::Core | Self::Filtered | Self::Sorted | Self::Paginated | Self::Final
        )
    }
}

/// Full row-model vocabulary order.
pub const TABLE_ROW_MODEL_PIPELINE: [TableRowModelStage; 7] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Grouped,
    TableRowModelStage::Sorted,
    TableRowModelStage::Expanded,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

/// Original v0 row-model subset.
pub const TABLE_ROW_MODEL_V0_PIPELINE: [TableRowModelStage; 5] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Sorted,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TableRowNode {
    pub(super) row: TableResolvedRow,
    pub(super) children: Vec<TableRowNode>,
}

impl TableRowNode {
    pub(super) fn leaf(row: TableResolvedRow) -> Self {
        Self {
            row,
            children: Vec::new(),
        }
    }
}

pub(super) fn build_source_row_nodes(
    rows: &[TableRow],
    selected_rows: &BTreeSet<TableRowId>,
    expansion: &TableExpansionState,
    include_children: bool,
    parent_id: Option<TableRowId>,
    depth: usize,
    source_index: &mut usize,
) -> Vec<TableRowNode> {
    rows.iter()
        .map(|row| {
            let current_source_index = *source_index;
            *source_index += 1;
            let loaded_child_count = row.children().len();
            let can_expand = row.can_expand();

            let children = if include_children {
                build_source_row_nodes(
                    row.children(),
                    selected_rows,
                    expansion,
                    include_children,
                    Some(row.id().clone()),
                    depth + 1,
                    source_index,
                )
            } else {
                Vec::new()
            };
            let tree = (include_children && (parent_id.is_some() || can_expand)).then(|| {
                TableTreeRow::new(
                    depth,
                    parent_id.clone(),
                    loaded_child_count > 0,
                    can_expand,
                    expansion.is_expanded(row.id()),
                    count_table_rows(row.children()),
                    loaded_child_count,
                    row.children_load_state().clone(),
                )
            });
            let resolved = TableResolvedRow::from_row(
                row,
                current_source_index,
                selected_rows.contains(row.id()),
                tree,
            );

            TableRowNode {
                row: resolved,
                children,
            }
        })
        .collect()
}

pub(super) fn filter_source_row_nodes(
    nodes: &[TableRowNode],
    filters: &[TableFilter],
    excluded_column: Option<&TableColumnId>,
) -> Vec<TableRowNode> {
    if filters.is_empty()
        || filters
            .iter()
            .all(|filter| excluded_column.is_some_and(|column| filter.column() == column))
    {
        return nodes.to_vec();
    }

    nodes
        .iter()
        .filter_map(|node| {
            let source = node.row.source()?;
            if !filters.iter().all(|filter| {
                excluded_column.is_some_and(|column| filter.column() == column)
                    || filter.matches(source)
            }) {
                return None;
            }

            Some(TableRowNode {
                row: node.row.clone(),
                children: filter_source_row_nodes(&node.children, filters, excluded_column),
            })
        })
        .collect()
}

pub(super) fn filter_source_row_nodes_by_global_query(
    nodes: &[TableRowNode],
    global_filter: Option<&str>,
    global_filterable_columns: &[TableColumnId],
) -> Vec<TableRowNode> {
    if matches!(global_filter, None | Some("")) {
        return nodes.to_vec();
    }

    nodes
        .iter()
        .filter_map(|node| {
            let source = node.row.source()?;
            if !row_matches_global_filter(source, global_filter, global_filterable_columns) {
                return None;
            }

            Some(TableRowNode {
                row: node.row.clone(),
                children: filter_source_row_nodes_by_global_query(
                    &node.children,
                    global_filter,
                    global_filterable_columns,
                ),
            })
        })
        .collect()
}

pub(super) fn flatten_nodes(nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push(node.row.clone());
        rows.extend(flatten_nodes(&node.children));
    }
    rows
}

pub(super) fn build_group_nodes(
    rows: &[TableResolvedRow],
    grouping: &[TableColumnId],
    aggregations: &[TableAggregation],
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
    depth: usize,
    parent_group_id: Option<TableRowId>,
    inherited_parent_id: Option<TableRowId>,
) -> Vec<TableRowNode> {
    if grouping.is_empty() {
        return rows
            .iter()
            .cloned()
            .map(|row| {
                let row = match inherited_parent_id.as_ref() {
                    Some(parent_id) => row.with_parent(parent_id.clone(), depth),
                    None => row,
                };
                TableRowNode::leaf(row)
            })
            .collect();
    }

    let grouping_column = grouping[0].clone();
    let mut buckets: Vec<(String, TableCellValue, Vec<TableResolvedRow>)> = Vec::new();
    let mut bucket_index_by_key = BTreeMap::new();

    for row in rows {
        let value = row.cell(&grouping_column).cloned().unwrap_or_default();
        let key = value.filter_text();
        let index = match bucket_index_by_key.get(&key).copied() {
            Some(index) => index,
            None => {
                let index = buckets.len();
                bucket_index_by_key.insert(key.clone(), index);
                buckets.push((key.clone(), value.clone(), Vec::new()));
                index
            }
        };
        buckets[index].2.push(row.clone());
    }

    let mut nodes = Vec::new();
    for (value_text, value, bucket_rows) in buckets {
        let group_id = build_group_row_id(parent_group_id.as_ref(), &grouping_column, &value_text);
        let first_leaf_row_id = bucket_rows
            .first()
            .map(|row| row.id().clone())
            .unwrap_or_else(|| group_id.clone());
        let leaf_row_count = bucket_rows.len();
        let parent_id = inherited_parent_id.clone();
        let group = TableGroupRow::new(
            grouping_column.clone(),
            value,
            depth,
            parent_id.clone(),
            first_leaf_row_id,
            leaf_row_count,
        );
        let children = build_group_nodes(
            &bucket_rows,
            &grouping[1..],
            aggregations,
            aggregation_fns,
            depth + 1,
            Some(group_id.clone()),
            Some(group_id.clone()),
        );
        let aggregate_cells = resolve_aggregate_cells(&bucket_rows, aggregations, aggregation_fns);
        let row = TableResolvedRow::from_group(group_id, group, aggregate_cells);
        nodes.push(TableRowNode { row, children });
    }

    nodes
}

fn build_group_row_id(
    parent_id: Option<&TableRowId>,
    column: &TableColumnId,
    value_text: &str,
) -> TableRowId {
    let segment = format!("{}={}", column.as_str(), value_text);
    match parent_id {
        Some(parent) => TableRowId::new(format!("{}>{segment}", parent.as_str())),
        None => TableRowId::new(format!("group:{segment}")),
    }
}

pub(super) fn push_expanded_rows(
    node: &TableRowNode,
    expansion: &TableExpansionState,
    rows: &mut Vec<TableResolvedRow>,
) {
    rows.push(node.row.clone());
    if node.children.is_empty() {
        return;
    }

    if !expansion.is_expanded(node.row.id()) {
        return;
    }

    for child in &node.children {
        push_expanded_rows(child, expansion, rows);
    }
}
