use std::collections::BTreeMap;

use open_gpui::ElementId;
use open_gpui_ui_core::{
    TableColumnId, TableColumnRegion, TableResolvedHeaderCell, TableResolvedHeaderGroup,
    TableResolvedHeaderGroupRegions, TableResolvedHeaderKind, TableSortDirection, UiPx,
};

use super::super::TableHeaderAction;
use super::super::identity::{table_header_debug_selector, table_header_element_id};
use super::columns::TableColumnRenderPlan;

/// One resolved table header cell in render-plan form.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::table) struct TableHeaderCellRenderPlan {
    element_id: ElementId,
    debug_selector: String,
    label: String,
    kind: TableResolvedHeaderKind,
    row_span: usize,
    width: UiPx,
    leaf_column_ids: Vec<TableColumnId>,
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
        Self {
            element_id: table_header_element_id(table_id, cell.identity()),
            debug_selector: table_header_debug_selector(table_id, cell.identity()),
            label: cell.label().to_owned(),
            kind: cell.kind(),
            row_span: cell.row_span(),
            width,
            leaf_column_ids,
            sort_direction,
            sort_action,
            resizable,
        }
    }

    /// Returns the stable GPUI element identity for this header fragment.
    pub const fn element_id(&self) -> &ElementId {
        &self.element_id
    }

    /// Returns the stable diagnostic selector for this header cell.
    pub fn debug_selector(&self) -> &str {
        &self.debug_selector
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved header kind.
    pub const fn kind(&self) -> TableResolvedHeaderKind {
        self.kind
    }

    /// Returns the row span.
    pub const fn row_span(&self) -> usize {
        self.row_span
    }

    /// Returns the visible leaf ids covered by this cell.
    pub fn leaf_column_ids(&self) -> &[TableColumnId] {
        &self.leaf_column_ids
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
pub(in crate::table) struct TableHeaderGroupRenderPlan {
    depth: usize,
    headers: Vec<TableHeaderCellRenderPlan>,
}

impl TableHeaderGroupRenderPlan {
    fn from_resolved(
        table_id: &str,
        group: &TableResolvedHeaderGroup,
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let headers = group
            .headers()
            .iter()
            .map(|cell| TableHeaderCellRenderPlan::from_resolved(table_id, cell, columns_by_id))
            .collect::<Vec<_>>();
        Self {
            depth: group.depth(),
            headers,
        }
    }

    /// Returns the zero-based header row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the header cells in this row.
    pub fn headers(&self) -> &[TableHeaderCellRenderPlan] {
        &self.headers
    }
}

/// Header rows for one render region.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::table) struct TableHeaderGroupRegionRenderPlan {
    groups: Vec<TableHeaderGroupRenderPlan>,
}

impl TableHeaderGroupRegionRenderPlan {
    pub(in crate::table::render_plan) fn from_resolved(
        table_id: &str,
        groups: &[TableResolvedHeaderGroup],
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let groups = groups
            .iter()
            .map(|group| TableHeaderGroupRenderPlan::from_resolved(table_id, group, columns_by_id))
            .collect::<Vec<_>>();

        Self { groups }
    }

    /// Returns header rows in this region.
    pub fn groups(&self) -> &[TableHeaderGroupRenderPlan] {
        &self.groups
    }

    /// Returns the number of header rows in this region.
    pub fn header_row_count(&self) -> usize {
        self.groups.len()
    }
}

/// Header rows split into render regions.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::table) struct TableHeaderGroupRegionsRenderPlan {
    left: TableHeaderGroupRegionRenderPlan,
    center: TableHeaderGroupRegionRenderPlan,
    right: TableHeaderGroupRegionRenderPlan,
}

impl TableHeaderGroupRegionsRenderPlan {
    pub(in crate::table::render_plan) fn from_resolved(
        table_id: &str,
        header_groups: &TableResolvedHeaderGroupRegions,
        columns: &[TableColumnRenderPlan],
    ) -> Self {
        let columns_by_id = columns
            .iter()
            .map(|column| (column.id().clone(), column))
            .collect::<BTreeMap<_, _>>();
        Self {
            left: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                header_groups.left(),
                &columns_by_id,
            ),
            center: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                header_groups.center(),
                &columns_by_id,
            ),
            right: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                header_groups.right(),
                &columns_by_id,
            ),
        }
    }

    fn left(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.left
    }

    fn center(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.center
    }

    fn right(&self) -> &TableHeaderGroupRegionRenderPlan {
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
}
