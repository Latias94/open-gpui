use open_gpui_ui_core::{
    TableColumn, TableColumnId, TableColumnRegion, TableColumnSizing, TableSort,
    TableSortDirection, TableState, UiPx,
};

/// Relative placement for a controlled table column reorder change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableColumnOrderPlacement {
    /// Place the moved column before the target column.
    Before,
    /// Place the moved column after the target column.
    After,
}

impl TableColumnOrderPlacement {
    /// Returns a stable placement label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Controlled payload emitted when a table column reorder is committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnOrderChange {
    column_id: TableColumnId,
    target_column_id: TableColumnId,
    placement: TableColumnOrderPlacement,
    source_region: TableColumnRegion,
    target_region: TableColumnRegion,
}

impl TableColumnOrderChange {
    /// Creates a payload that moves one column before another within the same region.
    pub fn move_before(
        column_id: impl Into<TableColumnId>,
        target_column_id: impl Into<TableColumnId>,
        region: TableColumnRegion,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            target_column_id: target_column_id.into(),
            placement: TableColumnOrderPlacement::Before,
            source_region: region,
            target_region: region,
        }
    }

    /// Creates a payload that moves one column after another within the same region.
    pub fn move_after(
        column_id: impl Into<TableColumnId>,
        target_column_id: impl Into<TableColumnId>,
        region: TableColumnRegion,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            target_column_id: target_column_id.into(),
            placement: TableColumnOrderPlacement::After,
            source_region: region,
            target_region: region,
        }
    }

    /// Returns the moved column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the target column identity.
    pub const fn target_column_id(&self) -> &TableColumnId {
        &self.target_column_id
    }

    /// Returns the requested insertion placement.
    pub const fn placement(&self) -> TableColumnOrderPlacement {
        self.placement
    }

    /// Returns the source column region at drag start.
    pub const fn source_region(&self) -> TableColumnRegion {
        self.source_region
    }

    /// Returns the target column region at drop time.
    pub const fn target_region(&self) -> TableColumnRegion {
        self.target_region
    }

    /// Applies this reorder request to a table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        if self.source_region != self.target_region {
            return state;
        }

        let current_order = effective_table_column_order(&state);
        let Some(next_order) = reorder_table_column_order(
            current_order,
            self.column_id.clone(),
            self.target_column_id.clone(),
            self.placement,
        ) else {
            return state;
        };

        state.with_column_order(next_order)
    }
}

/// Sort request emitted by an interactive table column header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableHeaderAction {
    column_id: TableColumnId,
    label: String,
    current_direction: Option<TableSortDirection>,
    next_direction: Option<TableSortDirection>,
    next_sorting: Vec<TableSort>,
}

impl TableHeaderAction {
    pub(in crate::table) fn for_column(
        column: &TableColumn,
        current_direction: Option<TableSortDirection>,
    ) -> Self {
        let next_direction = match current_direction {
            None => Some(TableSortDirection::Ascending),
            Some(TableSortDirection::Ascending) => Some(TableSortDirection::Descending),
            Some(TableSortDirection::Descending) => None,
        };
        let next_sorting = next_direction
            .map(|direction| vec![TableSort::new(column.id().clone(), direction)])
            .unwrap_or_default();

        Self {
            column_id: column.id().clone(),
            label: column.label().to_owned(),
            current_direction,
            next_direction,
            next_sorting,
        }
    }

    /// Returns the activated column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the activated column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the currently resolved sort direction for the column.
    pub const fn current_direction(&self) -> Option<TableSortDirection> {
        self.current_direction
    }

    /// Returns the direction that should be applied by the next state update.
    pub const fn next_direction(&self) -> Option<TableSortDirection> {
        self.next_direction
    }

    /// Returns the next single-column sorting state.
    pub fn next_sorting(&self) -> &[TableSort] {
        &self.next_sorting
    }

    /// Applies this header action to a table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        state.with_sorting(self.next_sorting.clone())
    }
}

/// Controlled payload emitted when a table column resize commits.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnSizingChange {
    column_id: TableColumnId,
    width: UiPx,
    sizing: TableColumnSizing,
}

impl TableColumnSizingChange {
    /// Creates a committed resize payload.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        width: UiPx,
        sizing: TableColumnSizing,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            width,
            sizing,
        }
    }

    /// Returns the resized column id.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved column width for the resized column.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the next committed sizing map.
    pub const fn sizing(&self) -> &TableColumnSizing {
        &self.sizing
    }
}

fn effective_table_column_order(state: &TableState) -> Vec<TableColumnId> {
    state.normalized_column_order()
}

fn reorder_table_column_order(
    mut column_order: Vec<TableColumnId>,
    column_id: TableColumnId,
    target_column_id: TableColumnId,
    placement: TableColumnOrderPlacement,
) -> Option<Vec<TableColumnId>> {
    if column_id == target_column_id {
        return None;
    }

    let source_index = column_order.iter().position(|id| id == &column_id)?;
    let _ = column_order.remove(source_index);
    let target_index = column_order.iter().position(|id| id == &target_column_id)?;
    let insert_index = match placement {
        TableColumnOrderPlacement::Before => target_index,
        TableColumnOrderPlacement::After => target_index + 1,
    };
    column_order.insert(insert_index, column_id);

    Some(column_order)
}
