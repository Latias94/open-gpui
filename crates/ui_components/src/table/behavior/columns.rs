use open_gpui_ui_core::{
    TableCellEditor, TableColumnId, TableColumnRegion, TableColumnWidthPolicy, TableResolvedState,
    TableRowPinningPolicy, TableSelectOption, TableSortDirection, UiPx,
};

use crate::table::TableHeaderAction;
use crate::table::render_plan::{TableColumnRenderPlan, TableRenderPlan};
/// Visible column region summary without exposing render-plan columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableColumnRegionSnapshot {
    left_columns: usize,
    center_columns: usize,
    right_columns: usize,
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
    aria_columns: usize,
    resizable_columns: usize,
    row_pinning_policy: TableRowPinningPolicy,
}

impl TableColumnRegionSnapshot {
    pub(in crate::table::behavior) fn from_render_plan(
        plan: &TableRenderPlan,
        table: &TableResolvedState,
    ) -> Self {
        let regions = table.visible_column_regions();
        Self {
            left_columns: regions.left().len(),
            center_columns: regions.center().len(),
            right_columns: regions.right().len(),
            left_width: plan.column_region_width(TableColumnRegion::Left),
            center_width: plan.column_region_width(TableColumnRegion::Center),
            right_width: plan.column_region_width(TableColumnRegion::Right),
            total_width: plan.total_column_width(),
            aria_columns: plan.aria_column_count(),
            resizable_columns: plan
                .columns()
                .iter()
                .filter(|column| column.resizable())
                .count(),
            row_pinning_policy: table.row_pinning_policy(),
        }
    }

    /// Returns visible left-pinned column count.
    pub const fn left_columns(self) -> usize {
        self.left_columns
    }

    /// Returns visible unpinned center column count.
    pub const fn center_columns(self) -> usize {
        self.center_columns
    }

    /// Returns visible right-pinned column count.
    pub const fn right_columns(self) -> usize {
        self.right_columns
    }

    /// Returns rounded left-pinned lane width.
    pub const fn left_width(self) -> UiPx {
        self.left_width
    }

    /// Returns rounded center lane width.
    pub const fn center_width(self) -> UiPx {
        self.center_width
    }

    /// Returns rounded right-pinned lane width.
    pub const fn right_width(self) -> UiPx {
        self.right_width
    }

    /// Returns total visible column width.
    pub const fn total_width(self) -> UiPx {
        self.total_width
    }

    /// Returns the accessibility column count.
    pub const fn aria_columns(self) -> usize {
        self.aria_columns
    }

    /// Returns visible resizable column count.
    pub const fn resizable_columns(self) -> usize {
        self.resizable_columns
    }

    /// Returns pinned row visibility policy.
    pub const fn row_pinning_policy(self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns whether pinned rows are limited to the current page.
    pub const fn row_pinning_page_only(self) -> bool {
        matches!(self.row_pinning_policy, TableRowPinningPolicy::PageOnly)
    }

    /// Returns the width for one column region.
    pub fn width_for(self, region: TableColumnRegion) -> UiPx {
        match region {
            TableColumnRegion::Left => self.left_width,
            TableColumnRegion::Center => self.center_width,
            TableColumnRegion::Right => self.right_width,
        }
    }

    /// Returns whether the table has pinned columns that render in separate lanes.
    pub const fn uses_split_pinned_columns(self) -> bool {
        self.left_columns > 0 || self.right_columns > 0
    }
}

/// User-observable metadata for one visible table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnBehaviorSnapshot {
    id: TableColumnId,
    label: String,
    region: TableColumnRegion,
    aria_column_index: usize,
    sortable: bool,
    editor: Option<TableCellEditor>,
    select_options: Vec<TableSelectOption>,
    width_policy: TableColumnWidthPolicy,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
    width: UiPx,
    resizable: bool,
}

impl TableColumnBehaviorSnapshot {
    pub(in crate::table::behavior) fn from_plan(column: &TableColumnRenderPlan) -> Self {
        Self {
            id: column.id().clone(),
            label: column.label().to_owned(),
            region: column.region(),
            aria_column_index: column.aria_column_index(),
            sortable: column.sortable(),
            editor: column.editor(),
            select_options: column.select_options().to_vec(),
            width_policy: column.width_policy(),
            sort_direction: column.sort_direction(),
            sort_action: column.sort_action().cloned(),
            width: column.width(),
            resizable: column.resizable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved pinning region for this column.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns whether this column is sortable in the contract.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether leaf cells in this column render editors.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns the configured editor for leaf cells in this column.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns fixed select options configured for this column.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns the configured width policy for this column.
    pub const fn width_policy(&self) -> TableColumnWidthPolicy {
        self.width_policy
    }

    /// Returns the resolved sort direction for this column, when present.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the header action emitted when this sortable column is activated.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns the resolved column width.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    /// Returns the label exposed to assistive technology.
    pub fn accessible_label(&self) -> String {
        match self.sort_direction {
            Some(direction) => format!("{}, sorted {}", self.label, direction.as_str()),
            None => self.label.clone(),
        }
    }
}
