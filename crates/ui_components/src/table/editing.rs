use open_gpui_ui_core::{
    TableCellValue, TableColumnId, TableRow, TableRowId, TableRowIdentity, TableSourceRowIdentity,
    TableSourceRowLookup, TableState,
};

use super::TableRowAction;

/// Outcome from applying a controlled table cell edit to app-owned table state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditApplyOutcome {
    /// The matching source row and cell were updated.
    Updated,
    /// No source row matched the edit payload row id.
    RowNotFound,
    /// A business row id matched multiple source instances without a resolved target.
    AmbiguousRowId,
    /// An occurrence identity belongs to a replaced or reordered source snapshot.
    StaleRowIdentity,
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
            Self::AmbiguousRowId => "ambiguous-row-id",
            Self::StaleRowIdentity => "stale-row-identity",
            Self::CellNotFound => "cell-not-found",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableCellEditValues {
    column_id: TableColumnId,
    previous_value: TableCellValue,
    next_value: TableCellValue,
    previous_text: String,
    next_text: String,
}

impl TableCellEditValues {
    fn new(
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        let previous_value = previous_value.into();
        let next_value = next_value.into();
        Self {
            column_id: column_id.into(),
            previous_text: previous_value.filter_text(),
            next_text: next_value.filter_text(),
            previous_value,
            next_value,
        }
    }
}

/// Application-owned request to edit one exact source-row cell.
///
/// A request carries only caller-known identity and value data. Runtime callbacks use
/// [`TableCellEditChange`] instead because they also carry authoritative resolved row metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellEditRequest {
    identity: TableSourceRowIdentity,
    values: TableCellEditValues,
}

impl TableCellEditRequest {
    /// Creates a request for one exact source-row identity.
    ///
    /// Raw business IDs are intentionally rejected because they cannot identify duplicate rows.
    ///
    /// ```compile_fail
    /// use open_gpui_ui_components::TableCellEditRequest;
    ///
    /// let _request = TableCellEditRequest::new("row", "name", "before", "after");
    /// ```
    pub fn new(
        identity: TableSourceRowIdentity,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        Self {
            identity,
            values: TableCellEditValues::new(column_id, previous_value, next_value),
        }
    }

    /// Returns the exact source-row identity targeted by this request.
    pub const fn source_identity(&self) -> &TableSourceRowIdentity {
        &self.identity
    }

    /// Returns the caller-owned business id for the targeted source row.
    pub const fn source_row_id(&self) -> &TableRowId {
        self.identity.row_id()
    }

    /// Returns the stable edited column id.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.values.column_id
    }

    /// Returns the resolved value before the edit.
    pub const fn previous_value(&self) -> &TableCellValue {
        &self.values.previous_value
    }

    /// Returns the resolved text before the edit.
    pub fn previous_text(&self) -> &str {
        &self.values.previous_text
    }

    /// Returns the resolved value after the edit.
    pub const fn next_value(&self) -> &TableCellValue {
        &self.values.next_value
    }

    /// Returns the next controlled text value.
    pub fn next_text(&self) -> &str {
        &self.values.next_text
    }

    /// Applies this request to a table state and returns an inspectable outcome.
    pub fn apply_to(&self, state: TableState) -> (TableState, TableCellEditApplyOutcome) {
        apply_table_cell_edit(state, &self.identity, &self.values)
    }
}

/// Controlled payload emitted when an editable table cell changes.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellEditChange {
    action: TableRowAction,
    values: TableCellEditValues,
}

impl TableCellEditChange {
    pub(super) fn new(
        action: TableRowAction,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        Self {
            action,
            values: TableCellEditValues::new(column_id, previous_value, next_value),
        }
    }

    /// Returns common row metadata for the edited cell.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the authoritative resolved edited-row identity.
    pub const fn identity(&self) -> &TableRowIdentity {
        self.action.identity()
    }

    /// Returns the caller-owned business id for source-backed edits.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        self.action.source_row_id()
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
        &self.values.column_id
    }

    /// Returns the resolved value before the edit.
    pub const fn previous_value(&self) -> &TableCellValue {
        &self.values.previous_value
    }

    /// Returns the resolved text before the edit.
    pub fn previous_text(&self) -> &str {
        &self.values.previous_text
    }

    /// Returns the resolved value after the edit.
    pub const fn next_value(&self) -> &TableCellValue {
        &self.values.next_value
    }

    /// Returns the next controlled text value.
    pub fn next_text(&self) -> &str {
        &self.values.next_text
    }

    /// Applies this edit to a table state and returns an inspectable outcome.
    pub fn apply_to(&self, state: TableState) -> (TableState, TableCellEditApplyOutcome) {
        let Some(source_identity) = self.identity().source_identity() else {
            return (state, TableCellEditApplyOutcome::RowNotFound);
        };
        apply_table_cell_edit(state, source_identity, &self.values)
    }
}

fn apply_table_cell_edit(
    state: TableState,
    source_identity: &TableSourceRowIdentity,
    values: &TableCellEditValues,
) -> (TableState, TableCellEditApplyOutcome) {
    let target_source_index = match state.source_row_lookup(source_identity) {
        TableSourceRowLookup::Found { source_index } => source_index,
        TableSourceRowLookup::Missing => {
            return (state, TableCellEditApplyOutcome::RowNotFound);
        }
        TableSourceRowLookup::Ambiguous => {
            return (state, TableCellEditApplyOutcome::AmbiguousRowId);
        }
        TableSourceRowLookup::StaleSnapshot => {
            return (state, TableCellEditApplyOutcome::StaleRowIdentity);
        }
    };

    let mut outcome = TableCellEditApplyOutcome::RowNotFound;
    let mut source_index = 0;
    let rows = state
        .rows()
        .iter()
        .cloned()
        .map(|row| {
            apply_table_cell_edit_to_row(
                row,
                target_source_index,
                &mut source_index,
                &values.column_id,
                &values.next_value,
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

fn apply_table_cell_edit_to_row(
    mut row: TableRow,
    target_source_index: usize,
    source_index: &mut usize,
    column_id: &TableColumnId,
    next_value: &TableCellValue,
    outcome: &mut TableCellEditApplyOutcome,
) -> TableRow {
    let current_source_index = *source_index;
    *source_index += 1;
    let matches_target = target_source_index == current_source_index;
    if matches_target {
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
        .map(|child| {
            apply_table_cell_edit_to_row(
                child,
                target_source_index,
                source_index,
                column_id,
                next_value,
                outcome,
            )
        })
        .collect::<Vec<_>>();
    row = row.with_replaced_children(children);
    row
}
