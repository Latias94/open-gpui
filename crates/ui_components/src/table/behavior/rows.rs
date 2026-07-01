use open_gpui_ui_core::{
    Role, TableCellEditor, TableCellValue, TableColumnId, TableColumnRegion,
    TableRowChildrenLoadState, TableRowId, TableRowRegion, TableSelectOption, UiPx,
};

use crate::table::render_plan::{TableCellRenderPlan, TableRowRenderPlan};
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
    pub(in crate::table::behavior) fn from_plan(row: &TableRowRenderPlan) -> Self {
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
    pub(in crate::table::behavior) fn from_plan(cell: &TableCellRenderPlan) -> Self {
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
