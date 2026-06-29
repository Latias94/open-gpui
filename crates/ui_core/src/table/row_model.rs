//! Row-model stage, pagination, and expansion vocabulary.

use std::collections::BTreeSet;

use super::{TableResolvedRow, TableRowId};

/// Per-stage row-model ownership for client or manual control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStageMode {
    /// The table applies the stage locally.
    Client,
    /// The caller supplies the stage output snapshot.
    Manual,
}

impl TableStageMode {
    /// Returns whether the stage is caller-owned.
    pub const fn is_manual(self) -> bool {
        matches!(self, Self::Manual)
    }
}

impl Default for TableStageMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Pagination state for a table row model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablePagination {
    page_index: usize,
    page_size: usize,
    mode: TableStageMode,
    row_count: Option<usize>,
    page_count: Option<usize>,
}

impl TablePagination {
    /// Creates pagination state from a page index and page size.
    pub const fn new(page_index: usize, page_size: usize) -> Self {
        Self {
            page_index,
            page_size,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Creates manual pagination state from a page index, page size, and total row count.
    pub const fn manual(page_index: usize, page_size: usize, row_count: usize) -> Self {
        Self::new(page_index, page_size)
            .with_mode(TableStageMode::Manual)
            .with_row_count(row_count)
    }

    /// Returns pagination that keeps all rows.
    pub const fn disabled() -> Self {
        Self {
            page_index: 0,
            page_size: usize::MAX,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Returns the zero-based page index.
    pub const fn page_index(self) -> usize {
        self.page_index
    }

    /// Returns the same pagination state with the page index reset.
    pub const fn with_page_index(mut self, page_index: usize) -> Self {
        self.page_index = page_index;
        self
    }

    /// Returns the maximum number of rows per page.
    pub const fn page_size(self) -> usize {
        self.page_size
    }

    /// Returns the pagination ownership mode.
    pub const fn mode(self) -> TableStageMode {
        self.mode
    }

    /// Returns whether pagination is caller-owned.
    pub const fn is_manual(self) -> bool {
        self.mode.is_manual()
    }

    /// Returns the total row count when known.
    pub const fn row_count(self) -> Option<usize> {
        self.row_count
    }

    /// Returns the total page count when known or derivable.
    pub fn page_count(self) -> Option<usize> {
        if let Some(page_count) = self.page_count {
            return Some(page_count);
        }

        let row_count = self.row_count?;
        if self.page_size == 0 {
            return Some(0);
        }

        Some(row_count.div_ceil(self.page_size))
    }

    /// Applies pagination ownership mode.
    pub const fn with_mode(mut self, mode: TableStageMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies a total row count.
    pub const fn with_row_count(mut self, row_count: usize) -> Self {
        self.row_count = Some(row_count);
        self
    }

    /// Applies a total page count.
    pub const fn with_page_count(mut self, page_count: usize) -> Self {
        self.page_count = Some(page_count);
        self
    }

    pub(super) fn apply(self, rows: &[TableResolvedRow]) -> Vec<TableResolvedRow> {
        if self.is_manual() || self.page_size == usize::MAX {
            return rows.to_vec();
        }
        if self.page_size == 0 {
            return Vec::new();
        }

        let start = self.page_index.saturating_mul(self.page_size);
        rows.iter()
            .skip(start)
            .take(self.page_size)
            .cloned()
            .collect()
    }
}

impl Default for TablePagination {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Caller-owned expansion state for grouped table rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableExpansionState {
    /// Every group row is expanded.
    All,
    /// Only the listed stable row ids are expanded.
    Rows(BTreeSet<TableRowId>),
}

impl TableExpansionState {
    /// Returns an expansion state where every row is expanded.
    pub const fn all() -> Self {
        Self::All
    }

    /// Returns an expansion state for explicit row ids.
    pub fn rows(rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        Self::Rows(rows.into_iter().map(Into::into).collect())
    }

    /// Returns whether the given row id should be expanded.
    pub fn is_expanded(&self, row_id: &TableRowId) -> bool {
        match self {
            Self::All => true,
            Self::Rows(rows) => rows.contains(row_id),
        }
    }
}

impl Default for TableExpansionState {
    fn default() -> Self {
        Self::Rows(BTreeSet::new())
    }
}

/// Row expansion behavior for resolved table row models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableExpansionMode {
    /// The core row model hides descendants of collapsed rows.
    Client,
    /// The caller supplies the visible source-tree snapshot.
    Manual,
}

impl TableExpansionMode {
    /// Returns whether local row-model expansion pruning is enabled.
    pub const fn prunes_collapsed_rows(self) -> bool {
        matches!(self, Self::Client)
    }
}

impl Default for TableExpansionMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Row-model stage vocabulary for Open GPUI tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowModelStage {
    /// Materialized one-to-one data rows.
    Core,
    /// Filtered rows.
    Filtered,
    /// Grouped rows.
    Grouped,
    /// Sorted rows.
    Sorted,
    /// Expanded rows.
    Expanded,
    /// Paginated rows.
    Paginated,
    /// Final row model consumed by renderers.
    Final,
}

impl TableRowModelStage {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Filtered => "filtered",
            Self::Grouped => "grouped",
            Self::Sorted => "sorted",
            Self::Expanded => "expanded",
            Self::Paginated => "paginated",
            Self::Final => "final",
        }
    }

    /// Returns whether this stage belonged to the original v0 resolver subset.
    pub const fn implemented_in_v0(self) -> bool {
        matches!(
            self,
            Self::Core | Self::Filtered | Self::Sorted | Self::Paginated | Self::Final
        )
    }
}

/// Full row-model vocabulary order.
pub const TABLE_ROW_MODEL_PIPELINE: [TableRowModelStage; 7] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Grouped,
    TableRowModelStage::Sorted,
    TableRowModelStage::Expanded,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

/// Original v0 row-model subset.
pub const TABLE_ROW_MODEL_V0_PIPELINE: [TableRowModelStage; 5] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Sorted,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];
