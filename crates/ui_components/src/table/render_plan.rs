use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui_ui_core::{
    GridViewport2D, Role, TableCellEditor, TableCellValue, TableColumn, TableColumnFacets,
    TableColumnId, TableColumnRegion, TableColumnWidthPolicy, TableGlobalFacetSummary,
    TableResolvedColumnSizing, TableResolvedHeaderCell, TableResolvedHeaderGroup,
    TableResolvedHeaderKind, TableResolvedRow, TableResolvedState, TableRowChildrenLoadState,
    TableRowId, TableRowRegion, TableSelectOption, TableSelectionPolicy, TableSelectionSummary,
    TableSortDirection, TableStageMode, TableState, UiPx, VirtualizerItemKey,
    VirtualizerItemMeasurement, VirtualizerRange, VirtualizerResolvedState, VirtualizerState,
};

use crate::table::layout::resolve_column_region_render_plans;

use super::{
    TableHeaderAction, TableMetrics, TableRowMeasureMode, apply_table_content_fit_widths,
    nonnegative_px, row_render_key,
};

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
    after: UiPx,
    resizable: bool,
}

impl TableColumnRenderPlan {
    pub(super) fn new(
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
            after: sizing.after(),
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

    /// Returns whether leaf cells in this column render text editors.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
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

    /// Returns the lower width bound.
    pub const fn min_width(&self) -> UiPx {
        self.min_width
    }

    /// Returns the upper width bound.
    pub const fn max_width(&self) -> UiPx {
        self.max_width
    }

    /// Returns the offset from the start edge of this column's region.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the offset from the end edge of this column's region.
    pub const fn after(&self) -> UiPx {
        self.after
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

    pub(super) fn with_width(mut self, width: UiPx) -> Self {
        self.width = width.max(self.min_width).min(self.max_width);
        self
    }

    pub(super) fn with_offsets(mut self, start: UiPx, after: UiPx) -> Self {
        self.start = start;
        self.after = after;
        self
    }

    #[cfg(test)]
    pub(super) fn test_content_fit_column(
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
            after: UiPx::ZERO,
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
    pub(super) fn new(region: TableColumnRegion, columns: Vec<TableColumnRenderPlan>) -> Self {
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
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
}

impl TablePinnedLayoutPlan {
    pub(super) fn from_column_regions(
        table_id: &str,
        regions: &[TableColumnRegionRenderPlan],
        total_width: UiPx,
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
            left_width: left
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            center_width: center
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            right_width: right
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            total_width,
        })
    }

    /// Returns the table identity this layout plan belongs to.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the total width of the left pinned lane.
    pub const fn left_width(&self) -> UiPx {
        self.left_width
    }

    /// Returns the total width of the horizontally scrollable center lane.
    pub const fn center_width(&self) -> UiPx {
        self.center_width
    }

    /// Returns the total width of the right pinned lane.
    pub const fn right_width(&self) -> UiPx {
        self.right_width
    }

    /// Returns the total width across all visible lanes.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the stable adapter id for the header center scroll viewport.
    pub fn header_center_scroll_id(&self) -> String {
        format!("table:{}:header-center-scroll", self.table_id)
    }

    /// Returns the stable debug selector for the header center scroll viewport.
    pub fn header_center_scroll_selector(&self) -> String {
        format!("scroll-area:{}", self.header_center_scroll_id())
    }

    /// Returns the stable debug selector for one header region lane.
    pub fn header_region_selector(&self, region: TableColumnRegion) -> String {
        format!("table:{}:header-region:{}", self.table_id, region.as_str())
    }

    /// Returns the stable adapter id for one body-row center scroll viewport.
    pub fn row_center_scroll_id(&self, row_render_key: &str) -> String {
        format!("table:{}:row-center-scroll:{row_render_key}", self.table_id)
    }

    /// Returns the stable debug selector for one body-row center scroll viewport.
    pub fn row_center_scroll_selector(&self, row_render_key: &str) -> String {
        format!("scroll-area:{}", self.row_center_scroll_id(row_render_key))
    }

    /// Returns the stable debug selector for one body-row region lane.
    pub fn row_region_selector(&self, row_render_key: &str, region: TableColumnRegion) -> String {
        format!(
            "table:{}:row-region:{row_render_key}:{}",
            self.table_id,
            region.as_str()
        )
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

    /// Returns the visible center-column range before overscan.
    pub const fn visible_range(&self) -> &VirtualizerRange {
        self.virtualizer.visible_range()
    }

    /// Returns the rendered center-column range after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        self.virtualizer.overscan_range()
    }

    /// Returns the rendered center columns in window order.
    pub fn rendered_columns(&self) -> &[TableColumnRenderPlan] {
        &self.rendered_columns
    }

    /// Returns the rendered center column count.
    pub fn rendered_column_count(&self) -> usize {
        self.rendered_columns.len()
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
    pub fn virtualized(&self) -> bool {
        self.rendered_columns.len() < self.virtualizer.count()
    }

    /// Returns the resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }
}

/// One resolved table header cell in render-plan form.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderCellRenderPlan {
    id: String,
    label: String,
    region: TableColumnRegion,
    depth: usize,
    index: usize,
    kind: TableResolvedHeaderKind,
    col_span: usize,
    row_span: usize,
    width: UiPx,
    start: UiPx,
    leaf_column_ids: Vec<TableColumnId>,
    sub_header_ids: Vec<String>,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
    resizable: bool,
}

impl TableHeaderCellRenderPlan {
    fn from_resolved(
        table_id: &str,
        cell: &TableResolvedHeaderCell,
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let leaf_column_ids = cell.leaf_column_ids().to_vec();
        let width = leaf_column_ids.iter().fold(UiPx::ZERO, |total, column_id| {
            total
                + columns_by_id
                    .get(column_id)
                    .copied()
                    .map(|column| column.width())
                    .unwrap_or(UiPx::ZERO)
        });
        let sort_source = leaf_column_ids
            .first()
            .and_then(|column_id| columns_by_id.get(column_id).copied());
        let leaf_header = cell.kind().is_leaf() && leaf_column_ids.len() == 1;
        let sort_direction = leaf_header
            .then(|| sort_source.and_then(|column| column.sort_direction()))
            .flatten();
        let sort_action = leaf_header
            .then(|| sort_source.and_then(|column| column.sort_action().cloned()))
            .flatten();
        let resizable = leaf_header
            .then(|| sort_source.map(|column| column.resizable()))
            .flatten()
            .unwrap_or(false);
        let start = leaf_column_ids
            .first()
            .and_then(|column_id| columns_by_id.get(column_id).copied())
            .map(TableColumnRenderPlan::start)
            .unwrap_or(UiPx::ZERO);

        Self {
            id: header_cell_render_id(table_id, cell),
            label: cell.label().to_owned(),
            region: cell.region(),
            depth: cell.depth(),
            index: cell.index(),
            kind: cell.kind(),
            col_span: cell.col_span(),
            row_span: cell.row_span(),
            width,
            start,
            leaf_column_ids,
            sub_header_ids: cell.sub_header_ids().to_vec(),
            sort_direction,
            sort_action,
            resizable,
        }
    }

    /// Returns the stable render identity for this header cell.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the render region for this header cell.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the header row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the index within the row.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the resolved header kind.
    pub const fn kind(&self) -> TableResolvedHeaderKind {
        self.kind
    }

    /// Returns the leaf-column span.
    pub const fn col_span(&self) -> usize {
        self.col_span
    }

    /// Returns the row span.
    pub const fn row_span(&self) -> usize {
        self.row_span
    }

    /// Returns the summed width of visible leaf coverage.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the horizontal start offset within the render lane.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the visible leaf ids covered by this cell.
    pub fn leaf_column_ids(&self) -> &[TableColumnId] {
        &self.leaf_column_ids
    }

    /// Returns direct child header ids.
    pub fn sub_header_ids(&self) -> &[String] {
        &self.sub_header_ids
    }

    /// Returns the resolved sort direction for leaf headers.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the emitted sort action for leaf headers.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns whether this leaf header is resizable.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }
}

/// One header row in a render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRenderPlan {
    id: String,
    region: TableColumnRegion,
    depth: usize,
    headers: Vec<TableHeaderCellRenderPlan>,
    total_width: UiPx,
}

impl TableHeaderGroupRenderPlan {
    fn from_resolved(
        table_id: &str,
        region: TableColumnRegion,
        group: &TableResolvedHeaderGroup,
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let headers = group
            .headers()
            .iter()
            .map(|cell| TableHeaderCellRenderPlan::from_resolved(table_id, cell, columns_by_id))
            .collect::<Vec<_>>();
        let total_width = headers
            .iter()
            .fold(UiPx::ZERO, |total, header| total + header.width());

        Self {
            id: format!(
                "table:{}:header-group:{}:{}",
                table_id,
                region.as_str(),
                group.depth()
            ),
            region,
            depth: group.depth(),
            headers,
            total_width,
        }
    }

    /// Returns the stable render identity for this header row.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the render region for this header row.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the header cells in this row.
    pub fn headers(&self) -> &[TableHeaderCellRenderPlan] {
        &self.headers
    }

    /// Returns the summed width of this header row.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }
}

/// Header rows for one render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRegionRenderPlan {
    region: TableColumnRegion,
    groups: Vec<TableHeaderGroupRenderPlan>,
    total_width: UiPx,
}

impl TableHeaderGroupRegionRenderPlan {
    fn from_resolved(
        table_id: &str,
        region: TableColumnRegion,
        groups: &[TableResolvedHeaderGroup],
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
        total_width: UiPx,
    ) -> Self {
        let groups = groups
            .iter()
            .map(|group| {
                TableHeaderGroupRenderPlan::from_resolved(table_id, region, group, columns_by_id)
            })
            .collect::<Vec<_>>();

        Self {
            region,
            groups,
            total_width,
        }
    }

    /// Returns the render region.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns header rows in this region.
    pub fn groups(&self) -> &[TableHeaderGroupRenderPlan] {
        &self.groups
    }

    /// Returns the number of header rows in this region.
    pub fn header_row_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns the summed width of this region.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the header row at a given depth, if present.
    pub fn group_at_depth(&self, depth: usize) -> Option<&TableHeaderGroupRenderPlan> {
        self.groups.iter().find(|group| group.depth() == depth)
    }
}

/// Header rows split into render regions.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRegionsRenderPlan {
    left: TableHeaderGroupRegionRenderPlan,
    center: TableHeaderGroupRegionRenderPlan,
    right: TableHeaderGroupRegionRenderPlan,
}

impl TableHeaderGroupRegionsRenderPlan {
    fn from_resolved(
        table_id: &str,
        header_groups: &open_gpui_ui_core::TableResolvedHeaderGroupRegions,
        columns: &[TableColumnRenderPlan],
        column_regions: &[TableColumnRegionRenderPlan],
    ) -> Self {
        let columns_by_id = columns
            .iter()
            .map(|column| (column.id().clone(), column))
            .collect::<BTreeMap<_, _>>();
        let region_width = |region: TableColumnRegion| {
            column_regions
                .iter()
                .find(|plan| plan.region() == region)
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO)
        };

        Self {
            left: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Left,
                header_groups.left(),
                &columns_by_id,
                region_width(TableColumnRegion::Left),
            ),
            center: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Center,
                header_groups.center(),
                &columns_by_id,
                region_width(TableColumnRegion::Center),
            ),
            right: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Right,
                header_groups.right(),
                &columns_by_id,
                region_width(TableColumnRegion::Right),
            ),
        }
    }

    /// Returns the left-pinned header rows.
    pub fn left(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.left
    }

    /// Returns the center header rows.
    pub fn center(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.center
    }

    /// Returns the right-pinned header rows.
    pub fn right(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.right
    }

    /// Returns header rows for a region.
    pub fn region(&self, region: TableColumnRegion) -> &TableHeaderGroupRegionRenderPlan {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns the maximum header row count across regions.
    pub fn row_count(&self) -> usize {
        self.left
            .header_row_count()
            .max(self.center.header_row_count())
            .max(self.right.header_row_count())
    }

    /// Returns whether no header rows exist.
    pub fn is_empty(&self) -> bool {
        self.row_count() == 0
    }

    /// Returns a shared header row at the given depth for a region family.
    pub fn group_at_depth(
        &self,
        region: TableColumnRegion,
        depth: usize,
    ) -> Option<&TableHeaderGroupRenderPlan> {
        self.region(region).group_at_depth(depth)
    }
}

fn header_cell_render_id(table_id: &str, cell: &TableResolvedHeaderCell) -> String {
    match cell.kind() {
        TableResolvedHeaderKind::Leaf => {
            format!("table:{table_id}:header:{}", cell.source_id())
        }
        TableResolvedHeaderKind::Group => format!(
            "table:{table_id}:header-group:{}:{}:{}",
            cell.region().as_str(),
            cell.depth(),
            cell.source_id()
        ),
        TableResolvedHeaderKind::Placeholder => format!(
            "table:{table_id}:header-placeholder:{}:{}:{}",
            cell.region().as_str(),
            cell.depth(),
            cell.placeholder_id().unwrap_or(cell.source_id())
        ),
    }
}

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
    pub(super) fn new(
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

/// Fully resolved render contract for a concrete [`Table`] instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRenderPlan {
    table_id: String,
    label: String,
    metrics: TableMetrics,
    row_measure_mode: TableRowMeasureMode,
    table: Rc<TableResolvedState>,
    virtualizer: VirtualizerResolvedState,
    content_fit_widths: BTreeMap<TableColumnId, UiPx>,
    columns: Vec<TableColumnRenderPlan>,
    column_regions: Vec<TableColumnRegionRenderPlan>,
    header_groups: TableHeaderGroupRegionsRenderPlan,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_column_window: Option<TableCenterColumnWindowPlan>,
    grid_viewport: Option<GridViewport2D>,
    total_column_width: UiPx,
    filtering_mode: TableStageMode,
    sorting_mode: TableStageMode,
    pagination_mode: TableStageMode,
    pagination_row_count: Option<usize>,
    pagination_page_count: Option<usize>,
    faceting_mode: TableStageMode,
    selection_policy: TableSelectionPolicy,
    selection_summary: TableSelectionSummary,
    aggregation_fn_count: usize,
    top_rows: Vec<TableRowRenderPlan>,
    rows: Vec<TableRowRenderPlan>,
    bottom_rows: Vec<TableRowRenderPlan>,
    role: Role,
    header_row_role: Role,
    column_header_role: Role,
    cell_role: Role,
}

impl TableRenderPlan {
    pub(super) fn resolve(
        table_id: String,
        label: String,
        metrics: TableMetrics,
        row_measure_mode: TableRowMeasureMode,
        state: &TableState,
        table: Rc<TableResolvedState>,
        virtualizer: VirtualizerResolvedState,
        columns: Vec<TableColumnRenderPlan>,
        content_fit_widths: BTreeMap<TableColumnId, UiPx>,
        center_scroll_offset: Option<UiPx>,
        center_viewport_extent: Option<UiPx>,
        row_measurements: &BTreeMap<String, UiPx>,
    ) -> Self {
        let columns =
            apply_table_content_fit_widths(columns, &content_fit_widths, state.column_sizing());
        let column_regions = resolve_column_region_render_plans(&columns);
        let header_groups = TableHeaderGroupRegionsRenderPlan::from_resolved(
            &table_id,
            table.header_groups(),
            &columns,
            &column_regions,
        );
        let total_column_width = column_regions
            .iter()
            .fold(UiPx::ZERO, |total, region| total + region.total_width());
        let pinned_layout = TablePinnedLayoutPlan::from_column_regions(
            &table_id,
            &column_regions,
            total_column_width,
        );
        let center = column_regions
            .iter()
            .find(|plan| plan.region() == TableColumnRegion::Center);
        let center_column_window = center.and_then(|center| {
            let viewport_extent = center_viewport_extent.unwrap_or_else(|| center.total_width());
            TableCenterColumnWindowPlan::resolve(
                center.columns(),
                center_scroll_offset.unwrap_or(UiPx::ZERO),
                viewport_extent,
                metrics.overscan(),
            )
        });
        let grid_viewport = center_column_window.as_ref().map(|center_window| {
            GridViewport2D::new(virtualizer.clone(), center_window.virtualizer().clone())
        });
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let top_row_count = table.top_rows().len();
        let center_total_row_count = table.center_rows().len();
        let top_rows = row_render_plans(
            table.top_rows(),
            TableRowRegion::Top,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            0,
            UiPx::ZERO,
        );
        let rows = virtualized_center_row_render_plans(
            table.center_rows(),
            virtualizer.items(),
            &columns,
            &duplicate_row_ids,
            top_row_count,
        );
        let top_height = top_rows
            .iter()
            .fold(UiPx::ZERO, |total, row| total + row.virtual_size());
        let bottom_rows = row_render_plans(
            table.bottom_rows(),
            TableRowRegion::Bottom,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            top_row_count + center_total_row_count,
            top_height + virtualizer.total_size(),
        );
        let pagination = state.pagination();
        let selection_summary = table.final_selection_summary();

        Self {
            table_id,
            label,
            metrics,
            row_measure_mode,
            table,
            virtualizer,
            content_fit_widths,
            columns,
            column_regions,
            header_groups,
            pinned_layout,
            center_column_window,
            grid_viewport,
            total_column_width,
            filtering_mode: state.filtering_mode(),
            sorting_mode: state.sorting_mode(),
            pagination_mode: pagination.mode(),
            pagination_row_count: pagination.row_count(),
            pagination_page_count: pagination.page_count(),
            faceting_mode: state.faceting_mode(),
            selection_policy: state.selection_policy(),
            selection_summary,
            aggregation_fn_count: state.aggregation_fn_count(),
            top_rows,
            rows,
            bottom_rows,
            role: Role::Table,
            header_row_role: Role::Row,
            column_header_role: Role::ColumnHeader,
            cell_role: Role::Cell,
        }
    }

    /// Returns the stable table id.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the accessible table label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns the row height ownership mode.
    pub const fn row_measure_mode(&self) -> TableRowMeasureMode {
        self.row_measure_mode
    }

    /// Returns the resolved renderer-neutral table state.
    pub fn table(&self) -> &TableResolvedState {
        self.table.as_ref()
    }

    /// Returns whether filtering was resolved locally or supplied by the caller.
    pub const fn filtering_mode(&self) -> TableStageMode {
        self.filtering_mode
    }

    /// Returns whether sorting was resolved locally or supplied by the caller.
    pub const fn sorting_mode(&self) -> TableStageMode {
        self.sorting_mode
    }

    /// Returns whether pagination was resolved locally or supplied by the caller.
    pub const fn pagination_mode(&self) -> TableStageMode {
        self.pagination_mode
    }

    /// Returns the server-known total row count, when supplied.
    pub const fn pagination_row_count(&self) -> Option<usize> {
        self.pagination_row_count
    }

    /// Returns the explicit or derived total page count, when supplied.
    pub const fn pagination_page_count(&self) -> Option<usize> {
        self.pagination_page_count
    }

    /// Returns whether faceting was resolved locally or supplied by the caller.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns the row-selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns the final row-model selection summary.
    pub const fn selection_summary(&self) -> TableSelectionSummary {
        self.selection_summary
    }

    /// Returns the number of named custom aggregation callbacks registered on the table state.
    pub const fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fn_count
    }

    /// Returns resolved facet metadata for configured columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        self.table.column_facets()
    }

    /// Returns resolved facet metadata for one configured column.
    pub fn column_facet(&self, column: &TableColumnId) -> Option<&TableColumnFacets> {
        self.table.column_facet(column)
    }

    /// Returns resolved facet metadata for the global filter context.
    pub fn global_facet_summary(&self) -> &TableGlobalFacetSummary {
        self.table.global_facet_summary()
    }

    /// Returns the resolved renderer-neutral virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns visible columns in render order.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns the measured content-fit widths that informed this render plan.
    pub fn content_fit_widths(&self) -> &BTreeMap<TableColumnId, UiPx> {
        &self.content_fit_widths
    }

    /// Returns visible columns split into render regions.
    pub fn column_regions(&self) -> &[TableColumnRegionRenderPlan] {
        &self.column_regions
    }

    /// Returns nested header groups split into render regions.
    pub fn header_groups(&self) -> &TableHeaderGroupRegionsRenderPlan {
        &self.header_groups
    }

    /// Returns left-pinned header rows.
    pub fn left_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.left()
    }

    /// Returns center header rows.
    pub fn center_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.center()
    }

    /// Returns right-pinned header rows.
    pub fn right_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.right()
    }

    /// Returns the maximum header row count across all regions.
    pub fn header_row_count(&self) -> usize {
        self.header_groups.row_count()
    }

    /// Returns the total height reserved for the table header band.
    pub fn sticky_header_band_height(&self) -> UiPx {
        self.metrics.header_height() * self.header_row_count().max(1) as f32
    }

    /// Returns sticky pinned-column layout metadata, when a split layout is needed.
    pub fn pinned_layout(&self) -> Option<&TablePinnedLayoutPlan> {
        self.pinned_layout.as_ref()
    }

    /// Returns center-column window metadata, when the center lane exists.
    pub fn center_column_window(&self) -> Option<&TableCenterColumnWindowPlan> {
        self.center_column_window.as_ref()
    }

    /// Returns the combined row and center-column viewport when both axes are available.
    pub fn grid_viewport(&self) -> Option<&GridViewport2D> {
        self.grid_viewport.as_ref()
    }

    /// Returns whether this render plan needs split pinned-column layout.
    pub fn uses_split_pinned_layout(&self) -> bool {
        self.pinned_layout.is_some()
    }

    /// Returns the summed resolved width of all visible columns.
    pub const fn total_column_width(&self) -> UiPx {
        self.total_column_width
    }

    /// Returns the summed resolved width of one visible column region.
    pub fn column_region_width(&self, region: TableColumnRegion) -> UiPx {
        self.column_regions
            .iter()
            .find(|plan| plan.region() == region)
            .map(TableColumnRegionRenderPlan::total_width)
            .unwrap_or(UiPx::ZERO)
    }

    /// Returns top-pinned rows in render order.
    pub fn top_rows(&self) -> &[TableRowRenderPlan] {
        &self.top_rows
    }

    /// Returns virtualized center rows in render order.
    pub fn rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns virtualized center rows in render order.
    pub fn center_rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns bottom-pinned rows in render order.
    pub fn bottom_rows(&self) -> &[TableRowRenderPlan] {
        &self.bottom_rows
    }

    /// Returns all currently rendered rows in visual order.
    pub fn rendered_rows(&self) -> impl Iterator<Item = &TableRowRenderPlan> {
        self.top_rows
            .iter()
            .chain(self.rows.iter())
            .chain(self.bottom_rows.iter())
    }

    /// Returns the accessibility role for the table root.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.header_row_role
    }

    /// Returns the accessibility role for header cells.
    pub const fn column_header_role(&self) -> Role {
        self.column_header_role
    }

    /// Returns the accessibility role for body cells.
    pub const fn cell_role(&self) -> Role {
        self.cell_role
    }

    /// Returns the accessibility row count, including the header row.
    pub fn aria_row_count(&self) -> usize {
        self.table.final_model().rows().len().saturating_add(1)
    }

    /// Returns the accessibility column count.
    pub fn aria_column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the number of body rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.top_rows.len() + self.rows.len() + self.bottom_rows.len()
    }

    /// Returns the visible body row count before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.top_rows.len() + self.virtualizer.visible_items().len() + self.bottom_rows.len()
    }
}

fn virtualized_center_row_render_plans(
    rows: &[TableResolvedRow],
    measurements: &[VirtualizerItemMeasurement],
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
) -> Vec<TableRowRenderPlan> {
    measurements
        .iter()
        .filter_map(|measurement| {
            rows.get(measurement.index()).cloned().map(|row| {
                let render_key = row_render_key(&row, duplicate_row_ids);
                let model_index = model_index_start + measurement.index();
                TableRowRenderPlan::new(
                    row,
                    TableRowRegion::Center,
                    render_key,
                    model_index,
                    measurement.clone(),
                    columns,
                )
            })
        })
        .collect()
}

fn row_render_plans(
    rows: &[TableResolvedRow],
    region: TableRowRegion,
    row_measure_mode: TableRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    fallback_row_height: UiPx,
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
    start_offset: UiPx,
) -> Vec<TableRowRenderPlan> {
    let mut cursor = start_offset;
    rows.iter()
        .enumerate()
        .map(|(region_index, row)| {
            let row = row.clone();
            let render_key = row_render_key(&row, duplicate_row_ids);
            let model_index = model_index_start + region_index;
            let row_height = if row_measure_mode.measured() {
                row_measurements
                    .get(&render_key)
                    .copied()
                    .unwrap_or(fallback_row_height)
            } else {
                fallback_row_height
            };
            let measured =
                row_measure_mode.measured() && row_measurements.contains_key(&render_key);
            let measurement = VirtualizerItemMeasurement::new(
                region_index,
                VirtualizerItemKey::new(render_key.clone()),
                cursor,
                row_height,
                measured,
            );
            cursor = measurement.end();
            TableRowRenderPlan::new(row, region, render_key, model_index, measurement, columns)
        })
        .collect()
}
