//! Row-model stage, pagination, and expansion vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use super::aggregation::{TableAggregation, TableAggregationFn, resolve_aggregate_cells};
use super::faceting::row_matches_global_filter;
use super::filtering::TableFilter;
use super::resolved::{TableGroupRow, TableResolvedRow, TableTreeRow};
use super::rows::{TableRow, count_table_rows};
use super::{
    TableCellValue, TableColumnId, TableGroupRowIdentity, TableGroupValueIdentity, TableRowId,
    TableRowIdentity, TableRowIdentityDiagnostic, TableRowInstanceId, TableSourceInstanceIdentity,
    TableSourceRowIdentity,
};

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

/// Result of resolving one source-row identity against the current source snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSourceRowLookup {
    /// The identity resolved to one source row at this preorder index.
    Found {
        /// Zero-based preorder index in caller-owned source rows.
        source_index: usize,
    },
    /// No source row currently resolves to the identity.
    Missing,
    /// The target's caller-owned identity facts are not unique in this snapshot.
    Ambiguous,
    /// An occurrence identity belongs to an older caller-owned source snapshot.
    StaleSnapshot,
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
    Rows(BTreeSet<TableRowIdentity>),
}

impl TableExpansionState {
    /// Returns an expansion state where every row is expanded.
    pub const fn all() -> Self {
        Self::All
    }

    /// Returns an expansion state for exact resolved row identities.
    ///
    /// Raw business ids are intentionally rejected because they cannot identify duplicate source
    /// rows or synthetic group rows.
    ///
    /// ```compile_fail
    /// use open_gpui_ui_core::TableExpansionState;
    ///
    /// let _ = TableExpansionState::rows(["row-a"]);
    /// ```
    pub fn rows(rows: impl IntoIterator<Item = TableRowIdentity>) -> Self {
        Self::Rows(rows.into_iter().collect())
    }

    /// Returns whether the given resolved row identity should be expanded.
    pub fn is_expanded(&self, identity: &TableRowIdentity) -> bool {
        match self {
            Self::All => true,
            Self::Rows(rows) => rows.contains(identity),
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
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TableRowNode {
    pub(super) row: TableResolvedRow,
    pub(super) children: Vec<TableRowNode>,
}

#[derive(Debug)]
pub(super) struct TableSourceIdentityIndex {
    source_snapshot: u64,
    source_id_counts: BTreeMap<TableRowId, usize>,
    explicit_instance_counts: BTreeMap<(TableRowId, TableRowInstanceId), usize>,
    identities_by_source_index: Vec<TableSourceRowIdentity>,
    source_index_by_identity: BTreeMap<TableSourceRowIdentity, usize>,
    identities_by_row_id: BTreeMap<TableRowId, Vec<TableSourceRowIdentity>>,
}

impl TableSourceIdentityIndex {
    pub(super) fn new(rows: &[TableRow], source_snapshot: u64) -> Self {
        let mut index = Self {
            source_snapshot,
            source_id_counts: BTreeMap::new(),
            explicit_instance_counts: BTreeMap::new(),
            identities_by_source_index: Vec::with_capacity(count_table_rows(rows)),
            source_index_by_identity: BTreeMap::new(),
            identities_by_row_id: BTreeMap::new(),
        };
        index.record(rows);
        index.record_identities(rows, &mut BTreeMap::new());
        for (source_index, identity) in index.identities_by_source_index.iter().cloned().enumerate()
        {
            index
                .identities_by_row_id
                .entry(identity.row_id().clone())
                .or_default()
                .push(identity.clone());
            index
                .source_index_by_identity
                .insert(identity, source_index);
        }
        index
    }

    pub(super) const fn source_snapshot(&self) -> u64 {
        self.source_snapshot
    }

    pub(super) fn diagnostics(&self) -> Vec<TableRowIdentityDiagnostic> {
        self.source_id_counts
            .iter()
            .filter(|(_, occurrences)| **occurrences > 1)
            .map(
                |(row_id, occurrences)| TableRowIdentityDiagnostic::DuplicateRowId {
                    row_id: row_id.clone(),
                    occurrences: *occurrences,
                },
            )
            .chain(
                self.explicit_instance_counts
                    .iter()
                    .filter(|(_, occurrences)| **occurrences > 1)
                    .map(|((row_id, instance_id), occurrences)| {
                        TableRowIdentityDiagnostic::DuplicateSourceInstance {
                            row_id: row_id.clone(),
                            instance_id: instance_id.clone(),
                            occurrences: *occurrences,
                        }
                    }),
            )
            .collect()
    }

    pub(super) fn cursor(&self) -> TableSourceIdentityCursor<'_> {
        TableSourceIdentityCursor {
            index: self,
            source_index: 0,
        }
    }

    pub(super) fn lookup(&self, target: &TableSourceRowIdentity) -> TableSourceRowLookup {
        if let TableSourceInstanceIdentity::Occurrence(occurrence) = target.instance()
            && occurrence.source_snapshot() != self.source_snapshot
        {
            return TableSourceRowLookup::StaleSnapshot;
        }

        let ambiguous = match target.instance() {
            TableSourceInstanceIdentity::Unique => self
                .source_id_counts
                .get(target.row_id())
                .is_some_and(|count| *count > 1),
            TableSourceInstanceIdentity::Explicit(instance_id) => self
                .explicit_instance_counts
                .get(&(target.row_id().clone(), instance_id.clone()))
                .is_some_and(|count| *count > 1),
            TableSourceInstanceIdentity::Occurrence(_) => false,
        };
        if ambiguous {
            return TableSourceRowLookup::Ambiguous;
        }

        self.source_index_by_identity
            .get(target)
            .copied()
            .map(|source_index| TableSourceRowLookup::Found { source_index })
            .unwrap_or(TableSourceRowLookup::Missing)
    }

    pub(super) fn identity_at(
        &self,
        row_id: &TableRowId,
        occurrence: usize,
    ) -> Option<TableSourceRowIdentity> {
        self.identities_by_row_id
            .get(row_id)
            .and_then(|identities| identities.get(occurrence))
            .cloned()
    }

    fn record(&mut self, rows: &[TableRow]) {
        for row in rows {
            *self.source_id_counts.entry(row.id().clone()).or_default() += 1;
            if let Some(instance_id) = row.instance_id() {
                *self
                    .explicit_instance_counts
                    .entry((row.id().clone(), instance_id.clone()))
                    .or_default() += 1;
            }
            self.record(row.children());
        }
    }

    fn record_identities(
        &mut self,
        rows: &[TableRow],
        occurrences: &mut BTreeMap<TableRowId, usize>,
    ) {
        for row in rows {
            let occurrence = occurrences.entry(row.id().clone()).or_default();
            let identity = self.resolve(row, *occurrence);
            *occurrence += 1;
            self.identities_by_source_index.push(identity);
            self.record_identities(row.children(), occurrences);
        }
    }

    fn resolve(&self, row: &TableRow, occurrence: usize) -> TableSourceRowIdentity {
        match row.instance_id() {
            Some(instance_id)
                if self
                    .explicit_instance_counts
                    .get(&(row.id().clone(), instance_id.clone()))
                    .copied()
                    .unwrap_or(0)
                    == 1 =>
            {
                TableSourceRowIdentity::explicit(row.id().clone(), instance_id.clone())
            }
            _ if self.source_id_counts.get(row.id()).copied().unwrap_or(0) == 1 => {
                TableSourceRowIdentity::unique(row.id().clone())
            }
            _ => TableSourceRowIdentity::occurrence(
                row.id().clone(),
                self.source_snapshot,
                occurrence,
            ),
        }
    }
}

pub(super) struct TableSourceIdentityCursor<'a> {
    index: &'a TableSourceIdentityIndex,
    source_index: usize,
}

impl TableSourceIdentityCursor<'_> {
    fn resolve(&mut self) -> (usize, TableRowIdentity) {
        let source_index = self.source_index;
        self.source_index += 1;
        let identity = self.index.identities_by_source_index[source_index].clone();
        (source_index, TableRowIdentity::Source(identity))
    }

    fn advance(&mut self, rows: &[TableRow]) {
        self.source_index += count_table_rows(rows);
    }
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
    selected_rows: &BTreeSet<TableSourceRowIdentity>,
    propagate_selected_descendants: bool,
    ancestor_selected: bool,
    expansion: &TableExpansionState,
    include_children: bool,
    identity_cursor: &mut TableSourceIdentityCursor<'_>,
    parent_identity: Option<TableRowIdentity>,
    depth: usize,
) -> Vec<TableRowNode> {
    rows.iter()
        .map(|row| {
            let (current_source_index, identity) = identity_cursor.resolve();
            let loaded_child_count = row.children().len();
            let can_expand = row.can_expand();
            let selected = identity
                .source_identity()
                .is_some_and(|source| selected_rows.contains(source))
                || (propagate_selected_descendants && ancestor_selected);

            let children = if include_children {
                build_source_row_nodes(
                    row.children(),
                    selected_rows,
                    propagate_selected_descendants,
                    selected,
                    expansion,
                    include_children,
                    identity_cursor,
                    Some(identity.clone()),
                    depth + 1,
                )
            } else {
                identity_cursor.advance(row.children());
                Vec::new()
            };
            let tree = (include_children && (parent_identity.is_some() || can_expand)).then(|| {
                TableTreeRow::new(
                    depth,
                    parent_identity.clone(),
                    loaded_child_count > 0,
                    can_expand,
                    expansion.is_expanded(&identity),
                    count_table_rows(row.children()),
                    loaded_child_count,
                    row.children_load_state().clone(),
                )
            });
            let resolved =
                TableResolvedRow::from_row(row, identity, current_source_index, selected, tree);

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
    parent_group_identity: Option<TableGroupRowIdentity>,
    inherited_parent_identity: Option<TableRowIdentity>,
) -> Vec<TableRowNode> {
    if grouping.is_empty() {
        return rows
            .iter()
            .cloned()
            .map(|row| {
                let row = match inherited_parent_identity.as_ref() {
                    Some(parent_identity) => row.with_parent(parent_identity.clone(), depth),
                    None => row,
                };
                TableRowNode::leaf(row)
            })
            .collect();
    }

    let grouping_column = grouping[0].clone();
    let mut buckets: Vec<(TableCellValue, Vec<TableResolvedRow>)> = Vec::new();
    let mut bucket_index_by_key = BTreeMap::new();

    for row in rows {
        let value = row.cell(&grouping_column).cloned().unwrap_or_default();
        let key = TableGroupValueIdentity::from_cell_value(&value);
        let index = match bucket_index_by_key.get(&key).copied() {
            Some(index) => index,
            None => {
                let index = buckets.len();
                bucket_index_by_key.insert(key.clone(), index);
                buckets.push((value.clone(), Vec::new()));
                index
            }
        };
        buckets[index].1.push(row.clone());
    }

    let mut nodes = Vec::new();
    for (value, bucket_rows) in buckets {
        let group_identity = match parent_group_identity.as_ref() {
            Some(parent) => parent
                .clone()
                .child_cell_value(grouping_column.clone(), &value),
            None => TableGroupRowIdentity::from_cell_value(grouping_column.clone(), &value),
        };
        let resolved_identity = TableRowIdentity::group(group_identity.clone());
        let first_leaf_identity = bucket_rows
            .first()
            .map(|row| row.identity().clone())
            .unwrap_or_else(|| resolved_identity.clone());
        let leaf_row_count = bucket_rows.len();
        let parent_identity = inherited_parent_identity.clone();
        let group = TableGroupRow::new(
            grouping_column.clone(),
            value,
            depth,
            parent_identity.clone(),
            first_leaf_identity,
            leaf_row_count,
        );
        let children = build_group_nodes(
            &bucket_rows,
            &grouping[1..],
            aggregations,
            aggregation_fns,
            depth + 1,
            Some(group_identity),
            Some(resolved_identity.clone()),
        );
        let aggregate_cells = resolve_aggregate_cells(&bucket_rows, aggregations, aggregation_fns);
        let row = TableResolvedRow::from_group(resolved_identity, group, aggregate_cells);
        nodes.push(TableRowNode { row, children });
    }

    nodes
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

    if !expansion.is_expanded(node.row.identity()) {
        return;
    }

    for child in &node.children {
        push_expanded_rows(child, expansion, rows);
    }
}
