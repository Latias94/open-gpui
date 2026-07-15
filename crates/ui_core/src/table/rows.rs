//! Source rows and row-pinning contracts for renderer-neutral tables.

use std::collections::{BTreeMap, BTreeSet};

use super::TableRowIdentity;
use super::{
    TableCellValue, TableColumnId, TableResolvedRow, TableRowId, TableRowInstanceId, TableRowModel,
};

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
    top: Vec<TableRowPinTarget>,
    bottom: Vec<TableRowPinTarget>,
}

/// Caller-owned target for one row-pinning region.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableRowPinTarget {
    /// Pin exactly one authoritative source or synthetic row identity.
    Exact(TableRowIdentity),
    /// Pin every currently resolved source row with one business row id.
    AllSourceRows(TableRowId),
}

impl TableRowPinTarget {
    /// Creates an exact logical-row target.
    pub const fn exact(identity: TableRowIdentity) -> Self {
        Self::Exact(identity)
    }

    /// Creates an explicit business-id bulk target.
    pub fn all_source_rows(row_id: impl Into<TableRowId>) -> Self {
        Self::AllSourceRows(row_id.into())
    }
}

impl From<TableRowIdentity> for TableRowPinTarget {
    fn from(value: TableRowIdentity) -> Self {
        Self::Exact(value)
    }
}

impl TableRowPinning {
    /// Creates an empty row pinning state.
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(super) fn from_raw(
        top: impl IntoIterator<Item = impl Into<TableRowPinTarget>>,
        bottom: impl IntoIterator<Item = impl Into<TableRowPinTarget>>,
    ) -> Self {
        Self {
            top: top.into_iter().map(Into::into).collect(),
            bottom: bottom.into_iter().map(Into::into).collect(),
        }
    }

    /// Applies top-pinned row targets in caller-owned region order.
    pub fn pinned_top(
        mut self,
        targets: impl IntoIterator<Item = impl Into<TableRowPinTarget>>,
    ) -> Self {
        self.top = unique_row_pin_targets(targets);
        self
    }

    /// Applies bottom-pinned row targets in caller-owned region order.
    pub fn pinned_bottom(
        mut self,
        targets: impl IntoIterator<Item = impl Into<TableRowPinTarget>>,
    ) -> Self {
        self.bottom = unique_row_pin_targets(targets);
        self
    }

    /// Returns top-pinned row targets in caller-owned order.
    pub fn top_targets(&self) -> &[TableRowPinTarget] {
        &self.top
    }

    /// Returns bottom-pinned row targets in caller-owned order.
    pub fn bottom_targets(&self) -> &[TableRowPinTarget] {
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
        expanded_model: &TableRowModel,
        paginated_model: &TableRowModel,
        pinning: &TableRowPinning,
        policy: TableRowPinningPolicy,
    ) -> Self {
        if pinning.is_empty() {
            return Self {
                top: Vec::new(),
                center: paginated_model.rows().to_vec(),
                bottom: Vec::new(),
            };
        }

        let lookup_model = match policy {
            TableRowPinningPolicy::KeepPinnedRows => expanded_model,
            TableRowPinningPolicy::PageOnly => paginated_model,
        };
        let lookup = TableRowPinLookup::new(lookup_model, pinning);
        let mut pinned_ids = BTreeSet::new();
        let top = resolve_row_pin_targets(pinning.top_targets(), &lookup, &mut pinned_ids);
        let bottom = resolve_row_pin_targets(pinning.bottom_targets(), &lookup, &mut pinned_ids);
        let center = paginated_model
            .rows()
            .iter()
            .filter(|row| !pinned_ids.contains(row.identity()))
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
    instance_id: Option<TableRowInstanceId>,
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
            instance_id: None,
            cells: BTreeMap::new(),
            children: Vec::new(),
            expandable: false,
            children_load_state: TableRowChildrenLoadState::Idle,
        }
    }

    /// Returns the caller-owned business row id.
    ///
    /// Business ids may repeat. Use [`Self::with_instance_id`] to provide stable disambiguation;
    /// exact expansion and pinning targets use the resolved [`TableRowIdentity`].
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns the caller-owned source-instance identity, when supplied.
    pub const fn instance_id(&self) -> Option<&TableRowInstanceId> {
        self.instance_id.as_ref()
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

    /// Applies a stable source-instance identity for duplicate business row ids.
    pub fn with_instance_id(mut self, instance_id: impl Into<TableRowInstanceId>) -> Self {
        self.instance_id = Some(instance_id.into());
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

fn unique_row_pin_targets(
    targets: impl IntoIterator<Item = impl Into<TableRowPinTarget>>,
) -> Vec<TableRowPinTarget> {
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .map(Into::into)
        .filter(|target| seen.insert(target.clone()))
        .collect()
}

struct TableRowPinLookup<'a> {
    model: &'a TableRowModel,
    source_rows: BTreeMap<TableRowId, Vec<&'a TableResolvedRow>>,
}

impl<'a> TableRowPinLookup<'a> {
    fn new(model: &'a TableRowModel, pinning: &TableRowPinning) -> Self {
        let mut source_rows = pinning
            .top_targets()
            .iter()
            .chain(pinning.bottom_targets())
            .filter_map(|target| match target {
                TableRowPinTarget::Exact(_) => None,
                TableRowPinTarget::AllSourceRows(row_id) => Some((row_id.clone(), Vec::new())),
            })
            .collect::<BTreeMap<_, _>>();

        if !source_rows.is_empty() {
            for row in model.rows() {
                if let Some(row_id) = row.source_row_id()
                    && let Some(rows) = source_rows.get_mut(row_id)
                {
                    rows.push(row);
                }
            }
        }

        Self { model, source_rows }
    }
}

fn resolve_row_pin_targets(
    targets: &[TableRowPinTarget],
    lookup: &TableRowPinLookup<'_>,
    seen: &mut BTreeSet<TableRowIdentity>,
) -> Vec<TableResolvedRow> {
    let mut resolved = Vec::new();
    for target in targets {
        let mut push = |row: &TableResolvedRow| {
            if seen.insert(row.identity().clone()) {
                resolved.push(row.clone());
            }
        };
        match target {
            TableRowPinTarget::Exact(identity) => {
                if let Some(row) = lookup.model.materialized_row(identity) {
                    push(row);
                }
            }
            TableRowPinTarget::AllSourceRows(row_id) => {
                if let Some(rows) = lookup.source_rows.get(row_id) {
                    rows.iter().copied().for_each(push);
                }
            }
        }
    }
    resolved
}

pub(super) fn count_table_rows(rows: &[TableRow]) -> usize {
    rows.iter()
        .map(|row| 1 + count_table_rows(row.children()))
        .sum()
}
