use open_gpui_ui_core::{
    TableCellEditor, TableColumn, TableColumnId, TableColumnRegion, TableColumnWidthPolicy,
    TableResolvedColumnSizing, TableSelectOption, TableSortDirection, UiPx, VirtualizerItemKey,
    VirtualizerItemMeasurement, VirtualizerResolvedState, VirtualizerState,
};

use super::super::{TableHeaderAction, nonnegative_px};

/// One resolved table column in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnRenderPlan {
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
    min_width: UiPx,
    max_width: UiPx,
    start: UiPx,
    resizable: bool,
}

impl TableColumnRenderPlan {
    pub(in crate::table) fn new(
        column: &TableColumn,
        sizing: &TableResolvedColumnSizing,
        region: TableColumnRegion,
        aria_column_index: usize,
        sort_direction: Option<TableSortDirection>,
    ) -> Self {
        debug_assert_eq!(sizing.region(), region);

        Self {
            id: column.id().clone(),
            label: column.label().to_owned(),
            region,
            aria_column_index,
            sortable: column.sortable(),
            editor: column.editor(),
            select_options: column.select_options().to_vec(),
            width_policy: column.width_policy(),
            sort_direction,
            sort_action: column
                .sortable()
                .then(|| TableHeaderAction::for_column(column, sort_direction)),
            width: sizing.width(),
            min_width: sizing.min_width(),
            max_width: sizing.max_width(),
            start: sizing.start(),
            resizable: sizing.resizable(),
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

    /// Returns the configured editor for leaf cells in this column.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns the fixed select options configured for this column.
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

    /// Returns the offset from the start edge of this column's region.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    pub(in crate::table) fn with_width(mut self, width: UiPx) -> Self {
        self.width = width.max(self.min_width).min(self.max_width);
        self
    }

    pub(in crate::table) fn with_start(mut self, start: UiPx) -> Self {
        self.start = start;
        self
    }

    #[cfg(test)]
    pub(in crate::table) fn test_content_fit_column(
        id: impl Into<TableColumnId>,
        label: impl Into<String>,
        width: UiPx,
        min_width: UiPx,
        max_width: UiPx,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            region: TableColumnRegion::Center,
            aria_column_index: 1,
            sortable: false,
            editor: None,
            select_options: Vec::new(),
            width_policy: TableColumnWidthPolicy::ContentFit,
            sort_direction: None,
            sort_action: None,
            width,
            min_width,
            max_width,
            start: UiPx::ZERO,
            resizable: true,
        }
    }
}

/// Resolved table columns for one render lane.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnRegionRenderPlan {
    region: TableColumnRegion,
    columns: Vec<TableColumnRenderPlan>,
    total_width: UiPx,
}

impl TableColumnRegionRenderPlan {
    pub(in crate::table) fn new(
        region: TableColumnRegion,
        columns: Vec<TableColumnRenderPlan>,
    ) -> Self {
        let total_width = columns
            .iter()
            .fold(UiPx::ZERO, |total, column| total + column.width());
        Self {
            region,
            columns,
            total_width,
        }
    }

    /// Returns the represented column region.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns columns in this region.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns the summed resolved width of columns in this region.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }
}

/// Adapter layout metadata for sticky pinned table column regions.
#[derive(Debug, Clone, PartialEq)]
pub struct TablePinnedLayoutPlan {
    table_id: String,
    center_width: UiPx,
}

impl TablePinnedLayoutPlan {
    pub(super) fn from_column_regions(
        table_id: &str,
        regions: &[TableColumnRegionRenderPlan],
        _total_width: UiPx,
    ) -> Option<Self> {
        let region_plan = |region| regions.iter().find(|plan| plan.region() == region);
        let left = region_plan(TableColumnRegion::Left);
        let center = region_plan(TableColumnRegion::Center);
        let right = region_plan(TableColumnRegion::Right);
        let has_pinned_columns = left
            .map(|region| !region.columns().is_empty())
            .unwrap_or(false)
            || right
                .map(|region| !region.columns().is_empty())
                .unwrap_or(false);
        if !has_pinned_columns {
            return None;
        }

        Some(Self {
            table_id: table_id.to_owned(),
            center_width: center
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
        })
    }

    /// Returns the stable adapter id for the header center scroll viewport.
    pub fn header_center_scroll_id(&self) -> String {
        format!("table:{}:header-center-scroll", self.table_id)
    }

    /// Returns the stable adapter id for one body-row center scroll viewport.
    pub fn row_center_scroll_id(&self, row_render_key: &str) -> String {
        format!("table:{}:row-center-scroll:{row_render_key}", self.table_id)
    }
}

/// Resolved render metadata for the virtualized center column lane.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCenterColumnWindowPlan {
    virtualizer: VirtualizerResolvedState,
    rendered_columns: Vec<TableColumnRenderPlan>,
    leading_spacer_width: UiPx,
    trailing_spacer_width: UiPx,
}

impl TableCenterColumnWindowPlan {
    /// Resolves a center-column virtual window from resolved center columns.
    pub fn resolve(
        columns: &[TableColumnRenderPlan],
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        overscan: usize,
    ) -> Option<Self> {
        if columns.is_empty() {
            return None;
        }

        let estimated_size = columns
            .first()
            .map(TableColumnRenderPlan::width)
            .unwrap_or(UiPx::ZERO);
        let virtualizer = VirtualizerState::new(columns.len(), estimated_size)
            .with_viewport_extent(nonnegative_px(viewport_extent))
            .with_scroll_offset(nonnegative_px(scroll_offset))
            .with_overscan(overscan)
            .resolve_known_size_window(|index| {
                let column = &columns[index];
                (
                    VirtualizerItemKey::new(column.id().as_str().to_owned()),
                    column.width(),
                )
            });
        let rendered_columns = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| columns.get(measurement.index()).cloned())
            .collect::<Vec<_>>();
        let leading_spacer_width = virtualizer
            .items()
            .first()
            .map(VirtualizerItemMeasurement::start)
            .unwrap_or(UiPx::ZERO);
        let trailing_spacer_width = virtualizer
            .items()
            .last()
            .map(|item| nonnegative_px(virtualizer.total_size() - item.end()))
            .unwrap_or(UiPx::ZERO);

        Some(Self {
            virtualizer,
            rendered_columns,
            leading_spacer_width,
            trailing_spacer_width,
        })
    }

    /// Returns the total width of the center lane.
    pub const fn center_width(&self) -> UiPx {
        self.virtualizer.total_size()
    }

    /// Returns the rendered center columns in window order.
    pub fn rendered_columns(&self) -> &[TableColumnRenderPlan] {
        &self.rendered_columns
    }

    /// Returns the leading spacer width before the first rendered center column.
    pub const fn leading_spacer_width(&self) -> UiPx {
        self.leading_spacer_width
    }

    /// Returns the trailing spacer width after the last rendered center column.
    pub const fn trailing_spacer_width(&self) -> UiPx {
        self.trailing_spacer_width
    }

    /// Returns whether the center lane is currently virtualized.
    #[cfg(test)]
    pub fn virtualized(&self) -> bool {
        self.rendered_columns.len() < self.virtualizer.count()
    }

    #[cfg(test)]
    pub const fn visible_range(&self) -> &open_gpui_ui_core::VirtualizerRange {
        self.virtualizer.visible_range()
    }

    #[cfg(test)]
    pub const fn overscan_range(&self) -> &open_gpui_ui_core::VirtualizerRange {
        self.virtualizer.overscan_range()
    }
}
