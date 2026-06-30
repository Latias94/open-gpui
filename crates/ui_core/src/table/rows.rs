//! Source rows and row-pinning contracts for renderer-neutral tables.

use std::collections::{BTreeMap, BTreeSet};

use super::{TableCellValue, TableColumnId, TableResolvedRow, TableRowId};

/// Resolved table row lane for row-pinning-aware renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableRowRegion {
    /// Rows pinned to the top body band.
    Top,
    /// Unpinned center rows.
    Center,
    /// Rows pinned to the bottom body band.
    Bottom,
}

impl TableRowRegion {
    /// All row regions in render order.
    pub const ALL: [Self; 3] = [Self::Top, Self::Center, Self::Bottom];

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

/// Policy for resolving pinned rows that are outside the current page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowPinningPolicy {
    /// Pinned rows may resolve from the expanded pre-pagination model.
    KeepPinnedRows,
    /// Pinned rows resolve only when they are present in the current page.
    PageOnly,
}

impl Default for TableRowPinningPolicy {
    fn default() -> Self {
        Self::KeepPinnedRows
    }
}

/// Caller-owned pinned row state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableRowPinning {
    top: Vec<TableRowId>,
    bottom: Vec<TableRowId>,
}

impl TableRowPinning {
    /// Creates an empty row pinning state.
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(super) fn from_raw(
        top: impl IntoIterator<Item = TableRowId>,
        bottom: impl IntoIterator<Item = TableRowId>,
    ) -> Self {
        Self {
            top: top.into_iter().collect(),
            bottom: bottom.into_iter().collect(),
        }
    }

    /// Applies top-pinned row ids.
    pub fn pinned_top(mut self, rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        self.top = unique_row_ids(rows);
        let top = self.top.iter().cloned().collect::<BTreeSet<_>>();
        self.bottom.retain(|row| !top.contains(row));
        self
    }

    /// Applies bottom-pinned row ids.
    pub fn pinned_bottom(mut self, rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        self.bottom = unique_row_ids(rows);
        let bottom = self.bottom.iter().cloned().collect::<BTreeSet<_>>();
        self.top.retain(|row| !bottom.contains(row));
        self
    }

    /// Returns top-pinned row ids.
    pub fn top(&self) -> &[TableRowId] {
        &self.top
    }

    /// Returns bottom-pinned row ids.
    pub fn bottom(&self) -> &[TableRowId] {
        &self.bottom
    }

    /// Returns true when no rows are pinned.
    pub fn is_empty(&self) -> bool {
        self.top.is_empty() && self.bottom.is_empty()
    }
}

/// Resolved visible rows split into row-pinning regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRowRegions {
    top: Vec<TableResolvedRow>,
    center: Vec<TableResolvedRow>,
    bottom: Vec<TableResolvedRow>,
}

impl TableRowRegions {
    pub(super) fn from_models(
        expanded_rows: &[TableResolvedRow],
        paginated_rows: &[TableResolvedRow],
        pinning: &TableRowPinning,
        policy: TableRowPinningPolicy,
    ) -> Self {
        if pinning.is_empty() {
            return Self {
                top: Vec::new(),
                center: paginated_rows.to_vec(),
                bottom: Vec::new(),
            };
        }

        let lookup_rows = match policy {
            TableRowPinningPolicy::KeepPinnedRows => expanded_rows,
            TableRowPinningPolicy::PageOnly => paginated_rows,
        };
        let mut top_seen = BTreeSet::new();
        let top_ids = pinning
            .top()
            .iter()
            .filter(|row_id| top_seen.insert((*row_id).clone()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let top = lookup_rows
            .iter()
            .filter(|row| top_ids.contains(row.id()))
            .cloned()
            .collect::<Vec<_>>();
        let top_ids = top
            .iter()
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();

        let mut bottom_seen = BTreeSet::new();
        let bottom_ids = pinning
            .bottom()
            .iter()
            .filter(|row_id| !top_ids.contains(*row_id))
            .filter(|row_id| bottom_seen.insert((*row_id).clone()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let bottom = lookup_rows
            .iter()
            .filter(|row| bottom_ids.contains(row.id()))
            .cloned()
            .collect::<Vec<_>>();
        let pinned_ids = top
            .iter()
            .chain(bottom.iter())
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();
        let center = paginated_rows
            .iter()
            .filter(|row| !pinned_ids.contains(row.id()))
            .cloned()
            .collect();

        Self {
            top,
            center,
            bottom,
        }
    }

    /// Returns top-pinned rows.
    pub fn top(&self) -> &[TableResolvedRow] {
        &self.top
    }

    /// Returns unpinned center rows.
    pub fn center(&self) -> &[TableResolvedRow] {
        &self.center
    }

    /// Returns bottom-pinned rows.
    pub fn bottom(&self) -> &[TableResolvedRow] {
        &self.bottom
    }

    /// Returns rows for a region.
    pub fn region(&self, region: TableRowRegion) -> &[TableResolvedRow] {
        match region {
            TableRowRegion::Top => self.top(),
            TableRowRegion::Center => self.center(),
            TableRowRegion::Bottom => self.bottom(),
        }
    }

    /// Returns the total number of visual body rows across all regions.
    pub fn len(&self) -> usize {
        self.top.len() + self.center.len() + self.bottom.len()
    }

    /// Returns true when all row regions are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn flattened(&self) -> Vec<TableResolvedRow> {
        self.top
            .iter()
            .chain(self.center.iter())
            .chain(self.bottom.iter())
            .cloned()
            .collect()
    }
}

/// Loading state for source row children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRowChildrenLoadState {
    /// No child load is currently pending or failed.
    Idle,
    /// Child rows are being loaded by the caller.
    Loading {
        /// Loading status text supplied by the caller.
        message: String,
    },
    /// Child row loading failed.
    Failed {
        /// Failure status text supplied by the caller.
        message: String,
    },
}

impl TableRowChildrenLoadState {
    /// Creates idle child loading metadata.
    pub const fn idle() -> Self {
        Self::Idle
    }

    /// Creates loading child metadata.
    pub fn loading(message: impl Into<String>) -> Self {
        Self::Loading {
            message: message.into(),
        }
    }

    /// Creates failed child loading metadata.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Returns whether child rows are currently loading.
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Returns whether child row loading failed.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns a stable loading-state label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading { .. } => "loading",
            Self::Failed { .. } => "failed",
        }
    }

    /// Returns the loading or failure message, when present.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::Loading { message } | Self::Failed { message } => Some(message.as_str()),
        }
    }
}

impl Default for TableRowChildrenLoadState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Renderer-neutral row descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    children: Vec<TableRow>,
    expandable: bool,
    children_load_state: TableRowChildrenLoadState,
}

impl TableRow {
    /// Creates a row with a stable identity.
    pub fn new(id: impl Into<TableRowId>) -> Self {
        Self {
            id: id.into(),
            cells: BTreeMap::new(),
            children: Vec::new(),
            expandable: false,
            children_load_state: TableRowChildrenLoadState::Idle,
        }
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns all cells keyed by column identity.
    pub const fn cells(&self) -> &BTreeMap<TableColumnId, TableCellValue> {
        &self.cells
    }

    /// Returns nested source rows.
    pub fn children(&self) -> &[TableRow] {
        &self.children
    }

    /// Returns whether this source row has nested children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Returns whether this source row can be expanded by the caller.
    pub fn can_expand(&self) -> bool {
        self.expandable
            || self.has_children()
            || !matches!(self.children_load_state, TableRowChildrenLoadState::Idle)
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TableRowChildrenLoadState {
        &self.children_load_state
    }

    /// Returns a cell value for the given column.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellValue> {
        self.cells.get(column)
    }

    /// Adds or replaces a cell value.
    pub fn with_cell(
        mut self,
        column: impl Into<TableColumnId>,
        value: impl Into<TableCellValue>,
    ) -> Self {
        self.cells.insert(column.into(), value.into());
        self
    }

    /// Adds one nested source row.
    pub fn with_child(mut self, child: TableRow) -> Self {
        self.children.push(child);
        self
    }

    /// Adds nested source rows.
    pub fn with_children(mut self, children: impl IntoIterator<Item = TableRow>) -> Self {
        self.children.extend(children);
        self
    }

    /// Replaces nested source rows.
    pub fn with_replaced_children(mut self, children: impl IntoIterator<Item = TableRow>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    /// Marks the row as expandable even when no child rows are currently loaded.
    pub const fn with_expandable(mut self, expandable: bool) -> Self {
        self.expandable = expandable;
        self
    }

    /// Applies caller-owned child loading metadata.
    pub fn with_children_load_state(mut self, state: TableRowChildrenLoadState) -> Self {
        if !matches!(state, TableRowChildrenLoadState::Idle) {
            self.expandable = true;
        }
        self.children_load_state = state;
        self
    }

    /// Marks child rows as currently loading.
    pub fn with_children_loading(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TableRowChildrenLoadState::loading(message))
    }

    /// Marks child row loading as failed.
    pub fn with_children_load_failed(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TableRowChildrenLoadState::failed(message))
    }
}

pub(super) fn unique_row_ids(
    rows: impl IntoIterator<Item = impl Into<TableRowId>>,
) -> Vec<TableRowId> {
    let mut seen = BTreeSet::new();
    rows.into_iter()
        .map(Into::into)
        .filter(|row| seen.insert(row.clone()))
        .collect()
}

pub(super) fn count_table_rows(rows: &[TableRow]) -> usize {
    rows.iter()
        .map(|row| 1 + count_table_rows(row.children()))
        .sum()
}
