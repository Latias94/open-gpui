//! Resolved table row and row-model state contracts.

use std::collections::BTreeMap;

use super::columns::{TableColumn, TableColumnRegions};
use super::faceting::{TableColumnFacets, TableGlobalFacetSummary};
use super::headers::{TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions};
use super::row_model::{TableRowModelStage, TableStageMode};
use super::rows::{TableRow, TableRowChildrenLoadState, TableRowPinningPolicy, TableRowRegions};
use super::selection::{TableSelectionPolicy, TableSelectionSummary};
use super::sizing::TableResolvedColumnSizingRegions;
use super::{TableCellValue, TableColumnId, TableRowId};

/// Metadata for a grouped table row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGroupRow {
    grouping_column: TableColumnId,
    grouping_value: TableCellValue,
    depth: usize,
    parent_id: Option<TableRowId>,
    first_leaf_row_id: TableRowId,
    leaf_row_count: usize,
}

impl TableGroupRow {
    pub(super) fn new(
        grouping_column: TableColumnId,
        grouping_value: TableCellValue,
        depth: usize,
        parent_id: Option<TableRowId>,
        first_leaf_row_id: TableRowId,
        leaf_row_count: usize,
    ) -> Self {
        Self {
            grouping_column,
            grouping_value,
            depth,
            parent_id,
            first_leaf_row_id,
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

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns the first descendant leaf row id.
    pub const fn first_leaf_row_id(&self) -> &TableRowId {
        &self.first_leaf_row_id
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
    parent_id: Option<TableRowId>,
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
        parent_id: Option<TableRowId>,
        has_children: bool,
        can_expand: bool,
        expanded: bool,
        descendant_count: usize,
        loaded_child_count: usize,
        children_load_state: TableRowChildrenLoadState,
    ) -> Self {
        Self {
            depth,
            parent_id,
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

    /// Returns the parent source row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
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
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    source: Option<TableRow>,
    source_index: Option<usize>,
    selected: bool,
    kind: TableResolvedRowKind,
    tree: Option<TableTreeRow>,
    depth: usize,
    parent_id: Option<TableRowId>,
}

impl TableResolvedRow {
    pub(super) fn from_row(
        row: &TableRow,
        source_index: usize,
        selected: bool,
        tree: Option<TableTreeRow>,
    ) -> Self {
        let depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
        let parent_id = tree.as_ref().and_then(|tree| tree.parent_id().cloned());
        Self {
            id: row.id().clone(),
            cells: row.cells().clone(),
            source: Some(row.clone()),
            source_index: Some(source_index),
            selected,
            kind: TableResolvedRowKind::Leaf,
            tree,
            depth,
            parent_id,
        }
    }

    pub(super) fn from_group(
        id: TableRowId,
        group: TableGroupRow,
        aggregate_cells: BTreeMap<TableColumnId, TableCellValue>,
    ) -> Self {
        let mut cells = aggregate_cells;
        cells.insert(
            group.grouping_column().clone(),
            group.grouping_value().clone(),
        );

        Self {
            id,
            cells,
            source: None,
            source_index: None,
            selected: false,
            depth: group.depth(),
            parent_id: group.parent_id().cloned(),
            kind: TableResolvedRowKind::Group(group),
            tree: None,
        }
    }

    pub(super) fn with_parent(mut self, parent_id: TableRowId, depth: usize) -> Self {
        self.parent_id = Some(parent_id);
        self.depth = depth;
        self
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        &self.id
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

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
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
    rows_by_id: BTreeMap<TableRowId, TableResolvedRow>,
}

impl TableRowModel {
    /// Creates a row model from rows at one stage.
    pub fn new(stage: TableRowModelStage, rows: impl Into<Vec<TableResolvedRow>>) -> Self {
        let rows = rows.into();
        Self::new_with_lookup(stage, rows.clone(), rows)
    }

    pub(super) fn new_with_lookup(
        stage: TableRowModelStage,
        rows: impl Into<Vec<TableResolvedRow>>,
        lookup_rows: impl IntoIterator<Item = TableResolvedRow>,
    ) -> Self {
        let rows = rows.into();
        let rows_by_id = lookup_rows
            .into_iter()
            .map(|row| (row.id().clone(), row))
            .collect();

        Self {
            stage,
            rows,
            rows_by_id,
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

    /// Returns the row lookup for this model.
    pub const fn rows_by_id(&self) -> &BTreeMap<TableRowId, TableResolvedRow> {
        &self.rows_by_id
    }

    /// Returns a row by stable id.
    pub fn row(&self, id: &TableRowId) -> Option<&TableResolvedRow> {
        self.rows_by_id.get(id)
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
    pub(super) duplicate_row_ids: Vec<TableRowId>,
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

    /// Returns duplicate source row ids detected during resolution.
    pub fn duplicate_row_ids(&self) -> &[TableRowId] {
        &self.duplicate_row_ids
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
