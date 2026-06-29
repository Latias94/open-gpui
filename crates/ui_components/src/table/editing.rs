use open_gpui_ui_core::{TableCellValue, TableColumnId, TableRow, TableRowId, TableState};

use super::TableRowAction;

/// Outcome from applying a controlled table cell edit to app-owned table state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditApplyOutcome {
    /// The matching source row and cell were updated.
    Updated,
    /// No source row matched the edit payload row id.
    RowNotFound,
    /// The source row exists, but the edited column does not exist on that row.
    CellNotFound,
}

impl TableCellEditApplyOutcome {
    /// Returns true when the state was updated.
    pub const fn updated(self) -> bool {
        matches!(self, Self::Updated)
    }

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::RowNotFound => "row-not-found",
            Self::CellNotFound => "cell-not-found",
        }
    }
}

/// Controlled payload emitted when an editable table cell changes.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellEditChange {
    action: TableRowAction,
    column_id: TableColumnId,
    previous_value: TableCellValue,
    next_value: TableCellValue,
    previous_text: String,
    next_text: String,
}

impl TableCellEditChange {
    pub(super) fn new(
        action: TableRowAction,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        let previous_value = previous_value.into();
        let next_value = next_value.into();
        Self {
            action,
            column_id: column_id.into(),
            previous_text: previous_value.filter_text(),
            next_text: next_value.filter_text(),
            previous_value,
            next_value,
        }
    }

    /// Creates an editable cell payload from stable row and column ids.
    pub fn for_row(
        row_id: impl Into<TableRowId>,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        let row_id = row_id.into();
        let previous_value = previous_value.into();
        let next_value = next_value.into();
        Self {
            action: TableRowAction::for_row(row_id),
            column_id: column_id.into(),
            previous_text: previous_value.filter_text(),
            next_text: next_value.filter_text(),
            previous_value,
            next_value,
        }
    }

    /// Returns common row metadata for the edited cell.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the stable edited row id.
    pub const fn row_id(&self) -> &TableRowId {
        self.action.row_id()
    }

    /// Returns the unique render key used by the edited row element.
    pub fn render_key(&self) -> &str {
        self.action.render_key()
    }

    /// Returns this row's zero-based index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.action.model_index()
    }

    /// Returns the source-row preorder index, when this is a source row.
    pub const fn source_index(&self) -> Option<usize> {
        self.action.source_index()
    }

    /// Returns the stable edited column id.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved value before the edit.
    pub const fn previous_value(&self) -> &TableCellValue {
        &self.previous_value
    }

    /// Returns the resolved text before the edit.
    pub fn previous_text(&self) -> &str {
        &self.previous_text
    }

    /// Returns the resolved value after the edit.
    pub const fn next_value(&self) -> &TableCellValue {
        &self.next_value
    }

    /// Returns the next controlled text value.
    pub fn next_text(&self) -> &str {
        &self.next_text
    }

    /// Applies this edit to a table state and returns an inspectable outcome.
    pub fn apply_to(&self, state: TableState) -> (TableState, TableCellEditApplyOutcome) {
        let mut outcome = TableCellEditApplyOutcome::RowNotFound;
        let rows = state
            .rows()
            .iter()
            .cloned()
            .map(|row| {
                apply_table_cell_edit_to_row(
                    row,
                    self.row_id(),
                    &self.column_id,
                    &self.next_value,
                    &mut outcome,
                )
            })
            .collect::<Vec<_>>();

        if outcome.updated() {
            (state.with_rows(rows), outcome)
        } else {
            (state, outcome)
        }
    }
}

fn apply_table_cell_edit_to_row(
    mut row: TableRow,
    row_id: &TableRowId,
    column_id: &TableColumnId,
    next_value: &TableCellValue,
    outcome: &mut TableCellEditApplyOutcome,
) -> TableRow {
    if row.id() == row_id {
        *outcome = if row.cell(column_id).is_some() {
            TableCellEditApplyOutcome::Updated
        } else {
            TableCellEditApplyOutcome::CellNotFound
        };

        if outcome.updated() {
            return row.with_cell(column_id.clone(), next_value.clone());
        }
        return row;
    }

    let children = row
        .children()
        .iter()
        .cloned()
        .map(|child| apply_table_cell_edit_to_row(child, row_id, column_id, next_value, outcome))
        .collect::<Vec<_>>();
    row = row.with_replaced_children(children);
    row
}
