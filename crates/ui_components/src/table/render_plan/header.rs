use std::collections::BTreeMap;

use open_gpui_ui_core::{
    TableColumnId, TableColumnRegion, TableResolvedHeaderCell, TableResolvedHeaderGroup,
    TableResolvedHeaderGroupRegions, TableResolvedHeaderKind, TableSortDirection, UiPx,
};

use super::super::TableHeaderAction;
use super::columns::{TableColumnRegionRenderPlan, TableColumnRenderPlan};

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
    pub(in crate::table::render_plan) fn from_resolved(
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
    pub(in crate::table::render_plan) fn from_resolved(
        table_id: &str,
        header_groups: &TableResolvedHeaderGroupRegions,
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
