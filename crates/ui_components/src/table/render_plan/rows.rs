use open_gpui_ui_core::{
    Role, TableCellEditor, TableCellValue, TableColumnId, TableColumnRegion, TableResolvedRow,
    TableRowChildrenLoadState, TableRowRegion, TableSelectOption, UiPx, VirtualizerItemMeasurement,
};

use super::columns::TableColumnRenderPlan;

/// One resolved table cell in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellRenderPlan {
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

impl TableCellRenderPlan {
    fn new(
        column: &TableColumnRenderPlan,
        row: &TableResolvedRow,
        value: Option<&TableCellValue>,
    ) -> Self {
        let value = value.cloned();
        let editor = if row.is_leaf() {
            match (column.editor(), value.as_ref()) {
                (Some(TableCellEditor::Checkbox), Some(TableCellValue::Bool(_))) => {
                    Some(TableCellEditor::Checkbox)
                }
                (Some(TableCellEditor::Select), Some(_)) => Some(TableCellEditor::Select),
                (Some(TableCellEditor::Text), Some(_))
                | (Some(TableCellEditor::MultilineText { .. }), Some(_)) => column.editor(),
                _ => None,
            }
        } else {
            None
        };
        let select_options = if matches!(editor, Some(TableCellEditor::Select)) {
            column.select_options().to_vec()
        } else {
            Vec::new()
        };
        let text = resolved_table_cell_text(value.as_ref(), &select_options);
        Self {
            column_id: column.id().clone(),
            value,
            text,
            select_options,
            region: column.region(),
            aria_column_index: column.aria_column_index(),
            role: Role::Cell,
            width: column.width(),
            editor,
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
    pub fn value(&self) -> Option<&TableCellValue> {
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

fn resolved_table_cell_text(
    value: Option<&TableCellValue>,
    select_options: &[TableSelectOption],
) -> String {
    let Some(value) = value else {
        return String::new();
    };

    let raw_text = value.filter_text();
    if select_options.is_empty() {
        return raw_text;
    }

    select_options
        .iter()
        .find(|option| option.value() == raw_text)
        .map(|option| option.label().to_owned())
        .unwrap_or(raw_text)
}

/// One resolved virtualized row to render.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowRenderPlan {
    row: TableResolvedRow,
    region: TableRowRegion,
    render_key: String,
    model_index: usize,
    aria_row_index: usize,
    measurement: VirtualizerItemMeasurement,
    cells: Vec<TableCellRenderPlan>,
    role: Role,
}

impl TableRowRenderPlan {
    pub(in crate::table) fn new(
        row: TableResolvedRow,
        region: TableRowRegion,
        render_key: String,
        model_index: usize,
        measurement: VirtualizerItemMeasurement,
        columns: &[TableColumnRenderPlan],
    ) -> Self {
        let cells = columns
            .iter()
            .map(|column| TableCellRenderPlan::new(column, &row, row.cell(column.id())))
            .collect();

        Self {
            row,
            region,
            render_key,
            model_index,
            aria_row_index: model_index + 2,
            measurement,
            cells,
            role: Role::Row,
        }
    }

    /// Returns the resolved core row.
    pub const fn row(&self) -> &TableResolvedRow {
        &self.row
    }

    /// Returns the stable row id.
    pub const fn id(&self) -> &open_gpui_ui_core::TableRowId {
        self.row.id()
    }

    /// Returns the row-pinning render region.
    pub const fn region(&self) -> TableRowRegion {
        self.region
    }

    /// Returns the unique render key used by element ids and virtualizer items.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns this row's index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.model_index
    }

    /// Returns this row's index inside its row-pinning region.
    pub const fn region_index(&self) -> usize {
        self.measurement.index()
    }

    /// Returns the 1-based accessibility row index, including the header row.
    pub const fn aria_row_index(&self) -> usize {
        self.aria_row_index
    }

    /// Returns whether the row is selected by stable row id.
    pub const fn selected(&self) -> bool {
        self.row.selected()
    }

    /// Returns this row's resolved hierarchy depth.
    pub const fn depth(&self) -> usize {
        self.row.depth()
    }

    /// Returns whether this rendered row is a source tree branch.
    pub fn is_tree_branch(&self) -> bool {
        self.row.is_tree_branch()
    }

    /// Returns the source tree expansion state for branch rows.
    pub fn tree_expanded(&self) -> Option<bool> {
        self.row.tree_expanded()
    }

    /// Returns the number of directly loaded child rows.
    pub fn loaded_child_count(&self) -> usize {
        self.row.loaded_child_count()
    }

    /// Returns source-row child loading metadata.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.row.children_load_state()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Returns the cells in visible column order.
    pub fn cells(&self) -> &[TableCellRenderPlan] {
        &self.cells
    }

    /// Returns cells for one column region.
    pub fn cells_for_region(
        &self,
        region: TableColumnRegion,
    ) -> impl Iterator<Item = &TableCellRenderPlan> {
        self.cells
            .iter()
            .filter(move |cell| cell.region() == region)
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}
