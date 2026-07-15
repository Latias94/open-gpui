//! Resolved table row and row-model state contracts.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::columns::{TableColumn, TableColumnRegions};
use super::faceting::{TableColumnFacets, TableGlobalFacetSummary};
use super::headers::{TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions};
use super::row_model::{TableRowModelStage, TableStageMode};
use super::rows::{TableRow, TableRowChildrenLoadState, TableRowPinningPolicy, TableRowRegions};
use super::selection::{TableSelectionPolicy, TableSelectionSummary};
use super::sizing::TableResolvedColumnSizingRegions;
use super::{
    TableCellValue, TableColumnId, TableRowId, TableRowIdentity, TableRowIdentityDiagnostic,
    TableRowIdentityKey,
};

/// Metadata for a grouped table row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGroupRow {
    grouping_column: TableColumnId,
    grouping_value: TableCellValue,
    depth: usize,
    parent_identity: Option<TableRowIdentity>,
    first_leaf_identity: TableRowIdentity,
    leaf_row_count: usize,
}

impl TableGroupRow {
    pub(super) fn new(
        grouping_column: TableColumnId,
        grouping_value: TableCellValue,
        depth: usize,
        parent_identity: Option<TableRowIdentity>,
        first_leaf_identity: TableRowIdentity,
        leaf_row_count: usize,
    ) -> Self {
        Self {
            grouping_column,
            grouping_value,
            depth,
            parent_identity,
            first_leaf_identity,
            leaf_row_count,
        }
    }

    /// Returns the grouped column identity.
    pub const fn grouping_column(&self) -> &TableColumnId {
        &self.grouping_column
    }

    /// Returns the grouped value.
    pub const fn grouping_value(&self) -> &TableCellValue {
        &self.grouping_value
    }

    /// Returns this group row's depth in the grouped tree.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent resolved row identity, if present.
    pub const fn parent_identity(&self) -> Option<&TableRowIdentity> {
        self.parent_identity.as_ref()
    }

    /// Returns the first descendant leaf row identity.
    pub const fn first_leaf_identity(&self) -> &TableRowIdentity {
        &self.first_leaf_identity
    }

    /// Returns the descendant leaf row count.
    pub const fn leaf_row_count(&self) -> usize {
        self.leaf_row_count
    }
}

/// Source hierarchy metadata for a resolved table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableTreeRow {
    depth: usize,
    parent_identity: Option<TableRowIdentity>,
    has_children: bool,
    can_expand: bool,
    expanded: bool,
    descendant_count: usize,
    loaded_child_count: usize,
    children_load_state: TableRowChildrenLoadState,
}

impl TableTreeRow {
    pub(super) fn new(
        depth: usize,
        parent_identity: Option<TableRowIdentity>,
        has_children: bool,
        can_expand: bool,
        expanded: bool,
        descendant_count: usize,
        loaded_child_count: usize,
        children_load_state: TableRowChildrenLoadState,
    ) -> Self {
        Self {
            depth,
            parent_identity,
            has_children,
            can_expand,
            expanded,
            descendant_count,
            loaded_child_count,
            children_load_state,
        }
    }

    /// Returns this source row's zero-based depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent source row identity, if present.
    pub const fn parent_identity(&self) -> Option<&TableRowIdentity> {
        self.parent_identity.as_ref()
    }

    /// Returns whether this source row has nested children.
    pub const fn has_children(&self) -> bool {
        self.has_children
    }

    /// Returns whether this source row can be expanded.
    pub const fn can_expand(&self) -> bool {
        self.can_expand
    }

    /// Returns whether this source branch is expanded in caller-owned state.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the number of nested descendant source rows.
    pub const fn descendant_count(&self) -> usize {
        self.descendant_count
    }

    /// Returns the number of directly loaded child rows.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TableRowChildrenLoadState {
        &self.children_load_state
    }
}

/// Resolved row kind for Open GPUI table row models.
#[derive(Debug, Clone, PartialEq)]
pub enum TableResolvedRowKind {
    /// A row backed by one source data row.
    Leaf,
    /// A synthetic grouped row.
    Group(TableGroupRow),
}

/// A resolved row that carries source identity and derived metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedRow {
    identity: TableRowIdentity,
    identity_key: TableRowIdentityKey,
    group_debug_label: Option<Arc<str>>,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    source: Option<TableRow>,
    source_index: Option<usize>,
    selected: bool,
    kind: TableResolvedRowKind,
    tree: Option<TableTreeRow>,
    depth: usize,
    parent_identity: Option<TableRowIdentity>,
}

impl TableResolvedRow {
    pub(super) fn from_row(
        row: &TableRow,
        identity: TableRowIdentity,
        source_index: usize,
        selected: bool,
        tree: Option<TableTreeRow>,
    ) -> Self {
        let identity_key = identity.key();
        let depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
        let parent_identity = tree
            .as_ref()
            .and_then(|tree| tree.parent_identity().cloned());
        Self {
            identity,
            identity_key,
            group_debug_label: None,
            cells: row.cells().clone(),
            source: Some(row.clone()),
            source_index: Some(source_index),
            selected,
            kind: TableResolvedRowKind::Leaf,
            tree,
            depth,
            parent_identity,
        }
    }

    pub(super) fn from_group(
        identity: TableRowIdentity,
        group: TableGroupRow,
        aggregate_cells: BTreeMap<TableColumnId, TableCellValue>,
    ) -> Self {
        let identity_key = identity.key();
        let group_debug_label = Arc::from(identity.debug_label());
        let mut cells = aggregate_cells;
        cells.insert(
            group.grouping_column().clone(),
            group.grouping_value().clone(),
        );

        Self {
            identity,
            identity_key,
            group_debug_label: Some(group_debug_label),
            cells,
            source: None,
            source_index: None,
            selected: false,
            depth: group.depth(),
            parent_identity: group.parent_identity().cloned(),
            kind: TableResolvedRowKind::Group(group),
            tree: None,
        }
    }

    pub(super) fn with_parent(mut self, parent_identity: TableRowIdentity, depth: usize) -> Self {
        self.parent_identity = Some(parent_identity);
        self.depth = depth;
        self
    }

    /// Returns the authoritative resolved row identity.
    pub const fn identity(&self) -> &TableRowIdentity {
        &self.identity
    }

    /// Returns the canonical encoded key shared by renderer projections.
    pub const fn identity_key(&self) -> &TableRowIdentityKey {
        &self.identity_key
    }

    /// Returns a human-readable row label for diagnostics, never identity lookup.
    pub fn debug_label(&self) -> &str {
        match &self.identity {
            TableRowIdentity::Source(source) => source.row_id().as_str(),
            TableRowIdentity::Group(_) => self
                .group_debug_label
                .as_deref()
                .expect("group rows always carry one shared diagnostic label"),
        }
    }

    /// Returns the caller-owned business row id for source-backed rows.
    pub fn source_row_id(&self) -> Option<&TableRowId> {
        self.identity.source_row_id()
    }

    /// Returns the resolved row kind.
    pub const fn kind(&self) -> &TableResolvedRowKind {
        &self.kind
    }

    /// Returns true when this is a grouped row.
    pub const fn is_group(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Group(_))
    }

    /// Returns true when this is a leaf source row.
    pub const fn is_leaf(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Leaf)
    }

    /// Returns grouped row metadata when this row is a group row.
    pub const fn group(&self) -> Option<&TableGroupRow> {
        match &self.kind {
            TableResolvedRowKind::Group(group) => Some(group),
            TableResolvedRowKind::Leaf => None,
        }
    }

    /// Returns source hierarchy metadata when this row came from source tree data.
    pub const fn tree(&self) -> Option<&TableTreeRow> {
        self.tree.as_ref()
    }

    /// Returns whether this row is a source row that can expand.
    pub fn is_tree_branch(&self) -> bool {
        self.tree().map(TableTreeRow::can_expand).unwrap_or(false)
    }

    /// Returns whether this source branch is expanded in caller-owned state.
    pub fn tree_expanded(&self) -> Option<bool> {
        self.tree()
            .filter(|tree| tree.can_expand())
            .map(TableTreeRow::expanded)
    }

    /// Returns the number of nested source descendants.
    pub fn descendant_count(&self) -> usize {
        self.tree().map(TableTreeRow::descendant_count).unwrap_or(0)
    }

    /// Returns the number of directly loaded child rows.
    pub fn loaded_child_count(&self) -> usize {
        self.tree()
            .map(TableTreeRow::loaded_child_count)
            .unwrap_or(0)
    }

    /// Returns caller-owned child loading metadata when this is a source-tree row.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.tree().map(TableTreeRow::children_load_state)
    }

    /// Returns the original row descriptor for leaf rows.
    pub const fn source(&self) -> Option<&TableRow> {
        self.source.as_ref()
    }

    /// Returns all resolved cells keyed by column identity.
    pub const fn cells(&self) -> &BTreeMap<TableColumnId, TableCellValue> {
        &self.cells
    }

    /// Returns a resolved cell value for the given column.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellValue> {
        self.cells.get(column)
    }

    /// Returns the original source index before row-model transforms for leaf rows.
    pub const fn source_index(&self) -> Option<usize> {
        self.source_index
    }

    /// Returns this row's depth in a grouped row model.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent resolved row identity, if present.
    pub const fn parent_identity(&self) -> Option<&TableRowIdentity> {
        self.parent_identity.as_ref()
    }

    /// Returns whether this row id is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Resolved rows for one row-model stage.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowModel {
    stage: TableRowModelStage,
    rows: Vec<TableResolvedRow>,
    row_lookup: BTreeMap<TableRowIdentity, TableRowLookupEntry>,
}

#[derive(Debug, Clone, PartialEq)]
enum TableRowLookupEntry {
    Materialized(usize),
    Retained(TableResolvedRow),
}

impl TableRowLookupEntry {
    fn row<'a>(&'a self, rows: &'a [TableResolvedRow]) -> &'a TableResolvedRow {
        match self {
            Self::Materialized(index) => &rows[*index],
            Self::Retained(row) => row,
        }
    }

    const fn materialized_index(&self) -> Option<usize> {
        match self {
            Self::Materialized(index) => Some(*index),
            Self::Retained(_) => None,
        }
    }
}

impl TableRowModel {
    /// Creates a row model from rows at one stage.
    pub fn new(stage: TableRowModelStage, rows: impl Into<Vec<TableResolvedRow>>) -> Self {
        let rows = rows.into();
        let row_lookup = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                (
                    row.identity().clone(),
                    TableRowLookupEntry::Materialized(index),
                )
            })
            .collect();

        Self {
            stage,
            rows,
            row_lookup,
        }
    }

    pub(super) fn new_with_lookup(
        stage: TableRowModelStage,
        rows: impl Into<Vec<TableResolvedRow>>,
        lookup_rows: impl IntoIterator<Item = TableResolvedRow>,
    ) -> Self {
        let rows = rows.into();
        let mut row_lookup = lookup_rows
            .into_iter()
            .map(|row| (row.identity().clone(), TableRowLookupEntry::Retained(row)))
            .collect::<BTreeMap<_, _>>();

        for (index, row) in rows.iter().enumerate() {
            row_lookup.insert(
                row.identity().clone(),
                TableRowLookupEntry::Materialized(index),
            );
        }

        Self {
            stage,
            rows,
            row_lookup,
        }
    }

    /// Returns this model's stage.
    pub const fn stage(&self) -> TableRowModelStage {
        self.stage
    }

    /// Returns rows in model order.
    pub fn rows(&self) -> &[TableResolvedRow] {
        &self.rows
    }

    /// Returns an identity's index in this stage's materialized row order.
    ///
    /// Unlike [`Self::row`], this excludes lookup-only rows retained from a pre-stage model.
    pub fn row_index(&self, identity: &TableRowIdentity) -> Option<usize> {
        self.row_lookup
            .get(identity)
            .and_then(TableRowLookupEntry::materialized_index)
    }

    /// Returns every addressable row, including lookup-only rows retained from a pre-stage model.
    ///
    /// Iteration follows identity order; use [`Self::rows`] for materialized model order.
    pub fn lookup_rows(&self) -> impl Iterator<Item = &TableResolvedRow> {
        self.row_lookup.values().map(|entry| entry.row(&self.rows))
    }

    /// Returns a row by authoritative resolved identity.
    pub fn row(&self, identity: &TableRowIdentity) -> Option<&TableResolvedRow> {
        self.row_lookup
            .get(identity)
            .map(|entry| entry.row(&self.rows))
    }

    /// Returns an exact row only when it is materialized in this stage's row order.
    pub(super) fn materialized_row(
        &self,
        identity: &TableRowIdentity,
    ) -> Option<&TableResolvedRow> {
        self.rows
            .get(self.row_lookup.get(identity)?.materialized_index()?)
    }

    /// Returns all source rows matching a caller-owned business id.
    pub fn source_rows<'a, 'b>(
        &'a self,
        row_id: &'b TableRowId,
    ) -> impl Iterator<Item = &'a TableResolvedRow> + use<'a, 'b> {
        // `Unique` is the first source-instance variant, so this starts the contiguous id range.
        self.row_lookup
            .range(TableRowIdentity::source(row_id.clone())..)
            .map(|(_, entry)| entry.row(&self.rows))
            .take_while(move |row| row.source_row_id() == Some(row_id))
    }

    /// Returns the source row only when the business id resolves uniquely.
    pub fn unique_source_row(&self, row_id: &TableRowId) -> Option<&TableResolvedRow> {
        let mut rows = self.source_rows(row_id);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }

    /// Returns the number of selected rows in this model.
    pub fn selected_count(&self) -> usize {
        self.rows.iter().filter(|row| row.selected()).count()
    }
}

/// Resolved table row models and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedState {
    pub(super) visible_columns: Vec<TableColumn>,
    pub(super) visible_column_regions: TableColumnRegions,
    pub(super) visible_column_sizing: TableResolvedColumnSizingRegions,
    pub(super) header_groups: TableResolvedHeaderGroupRegions,
    pub(super) row_identity_diagnostics: Vec<TableRowIdentityDiagnostic>,
    pub(super) faceting_mode: TableStageMode,
    pub(super) column_facets: Vec<TableColumnFacets>,
    pub(super) global_facet_summary: TableGlobalFacetSummary,
    pub(super) row_pinning_policy: TableRowPinningPolicy,
    pub(super) selection_policy: TableSelectionPolicy,
    pub(super) row_regions: TableRowRegions,
    pub(super) core_model: TableRowModel,
    pub(super) filtered_model: TableRowModel,
    pub(super) grouped_model: TableRowModel,
    pub(super) sorted_model: TableRowModel,
    pub(super) expanded_model: TableRowModel,
    pub(super) paginated_model: TableRowModel,
    pub(super) final_model: TableRowModel,
}

impl TableResolvedState {
    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> &[TableColumn] {
        &self.visible_columns
    }

    /// Returns visible columns split into pinned regions.
    pub const fn visible_column_regions(&self) -> &TableColumnRegions {
        &self.visible_column_regions
    }

    /// Returns resolved visible column sizing split into pinned regions.
    pub const fn visible_column_sizing(&self) -> &TableResolvedColumnSizingRegions {
        &self.visible_column_sizing
    }

    /// Returns resolved header groups split into pinned regions.
    pub const fn header_groups(&self) -> &TableResolvedHeaderGroupRegions {
        &self.header_groups
    }

    /// Returns resolved left-pinned header groups.
    pub fn left_header_groups(&self) -> &[TableResolvedHeaderGroup] {
        self.header_groups.left()
    }

    /// Returns resolved center header groups.
    pub fn center_header_groups(&self) -> &[TableResolvedHeaderGroup] {
        self.header_groups.center()
    }

    /// Returns resolved right-pinned header groups.
    pub fn right_header_groups(&self) -> &[TableResolvedHeaderGroup] {
        self.header_groups.right()
    }

    /// Returns the faceting ownership mode.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
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

    /// Returns the pinned row visibility policy.
    pub const fn row_pinning_policy(&self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns the row-selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns resolved row metadata for pinned and center regions.
    pub const fn row_regions(&self) -> &TableRowRegions {
        &self.row_regions
    }

    /// Returns top-pinned rows.
    pub fn top_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.top()
    }

    /// Returns center rows.
    pub fn center_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.center()
    }

    /// Returns bottom-pinned rows.
    pub fn bottom_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.bottom()
    }

    /// Returns source-row identity diagnostics detected during resolution.
    pub fn row_identity_diagnostics(&self) -> &[TableRowIdentityDiagnostic] {
        &self.row_identity_diagnostics
    }

    /// Returns the core row model.
    pub const fn core_model(&self) -> &TableRowModel {
        &self.core_model
    }

    /// Returns the filtered row model.
    pub const fn filtered_model(&self) -> &TableRowModel {
        &self.filtered_model
    }

    /// Returns the grouped row model.
    pub const fn grouped_model(&self) -> &TableRowModel {
        &self.grouped_model
    }

    /// Returns the sorted row model.
    pub const fn sorted_model(&self) -> &TableRowModel {
        &self.sorted_model
    }

    /// Returns the expanded row model.
    pub const fn expanded_model(&self) -> &TableRowModel {
        &self.expanded_model
    }

    /// Returns the paginated row model.
    pub const fn paginated_model(&self) -> &TableRowModel {
        &self.paginated_model
    }

    /// Returns the final row model consumed by renderers.
    pub const fn final_model(&self) -> &TableRowModel {
        &self.final_model
    }

    /// Returns the selection summary for the core row model.
    pub fn core_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.core_model.selected_count(),
            self.core_model.rows().len(),
        )
    }

    /// Returns the selection summary for the filtered row model.
    pub fn filtered_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.filtered_model.selected_count(),
            self.filtered_model.rows().len(),
        )
    }

    /// Returns the selection summary for the grouped row model.
    pub fn grouped_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.grouped_model.selected_count(),
            self.grouped_model.rows().len(),
        )
    }

    /// Returns the selection summary for the sorted row model.
    pub fn sorted_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.sorted_model.selected_count(),
            self.sorted_model.rows().len(),
        )
    }

    /// Returns the selection summary for the expanded row model.
    pub fn expanded_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.expanded_model.selected_count(),
            self.expanded_model.rows().len(),
        )
    }

    /// Returns the selection summary for the full resolved model before pagination.
    pub fn full_selection_summary(&self) -> TableSelectionSummary {
        self.core_selection_summary()
    }

    /// Returns the selection summary for the paginated row model.
    pub fn paginated_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.paginated_model.selected_count(),
            self.paginated_model.rows().len(),
        )
    }

    /// Returns the selection summary for the current page scope.
    pub fn current_page_selection_summary(&self) -> TableSelectionSummary {
        self.final_selection_summary()
    }

    /// Returns the selection summary for the final row model.
    pub fn final_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.final_model.selected_count(),
            self.final_model.rows().len(),
        )
    }
}
