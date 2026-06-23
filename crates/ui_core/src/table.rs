//! Renderer-neutral table row-model contracts for Open GPUI components.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::geometry::{UiPx, ui_px};

static NEXT_TABLE_ROWS_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Default preferred width for a table column.
pub const TABLE_DEFAULT_COLUMN_WIDTH: UiPx = ui_px(128.0);

/// Default minimum width for a table column.
pub const TABLE_MIN_COLUMN_WIDTH: UiPx = ui_px(40.0);

/// Default maximum width for a table column.
pub const TABLE_MAX_COLUMN_WIDTH: UiPx = ui_px(1_000_000.0);

/// Stable renderer-neutral identity for a table row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowId(String);

impl TableRowId {
    /// Creates a row identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableRowId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableRowId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Stable renderer-neutral identity for a table column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumnId(String);

impl TableColumnId {
    /// Creates a column identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableColumnId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableColumnId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Renderer-neutral scalar value used by table filtering and sorting.
#[derive(Debug, Clone, PartialEq)]
pub enum TableCellValue {
    /// No meaningful value is present.
    Empty,
    /// Text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
}

impl TableCellValue {
    /// Returns a stable string for filtering and debug output.
    pub fn filter_text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(value) => value.clone(),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }

    fn cmp_for_sort(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => left.total_cmp(right),
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Empty, Self::Empty) => Ordering::Equal,
            (Self::Empty, _) => Ordering::Less,
            (_, Self::Empty) => Ordering::Greater,
            _ => self.filter_text().cmp(&other.filter_text()),
        }
    }
}

impl Default for TableCellValue {
    fn default() -> Self {
        Self::Empty
    }
}

impl From<&str> for TableCellValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for TableCellValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<f64> for TableCellValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<i64> for TableCellValue {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<usize> for TableCellValue {
    fn from(value: usize) -> Self {
        Self::Number(value as f64)
    }
}

impl From<bool> for TableCellValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Renderer-neutral column descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    id: TableColumnId,
    label: String,
    visible: bool,
    sortable: bool,
    filterable: bool,
    width: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    resizable: bool,
}

impl TableColumn {
    /// Creates a visible, sortable, and filterable column descriptor.
    pub fn new(id: impl Into<TableColumnId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            visible: true,
            sortable: true,
            filterable: true,
            width: TABLE_DEFAULT_COLUMN_WIDTH,
            min_width: TABLE_MIN_COLUMN_WIDTH,
            max_width: TABLE_MAX_COLUMN_WIDTH,
            resizable: true,
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the human-readable column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this column should render by default.
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Returns whether this column accepts sorting.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether this column accepts filtering.
    pub const fn filterable(&self) -> bool {
        self.filterable
    }

    /// Returns the preferred width before committed sizing is applied.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the lower bound used when resolving this column's width.
    pub const fn min_width(&self) -> UiPx {
        self.min_width
    }

    /// Returns the upper bound used when resolving this column's width.
    pub const fn max_width(&self) -> UiPx {
        self.max_width
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    /// Applies column visibility.
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Applies sorting capability.
    pub const fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Applies filtering capability.
    pub const fn with_filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Applies the preferred width.
    pub fn with_width(mut self, width: UiPx) -> Self {
        self.width = normalized_column_width(width);
        self
    }

    /// Applies the minimum width.
    pub fn with_min_width(mut self, min_width: UiPx) -> Self {
        self.min_width = normalized_column_width(min_width);
        if self.max_width < self.min_width {
            self.max_width = self.min_width;
        }
        self
    }

    /// Applies the maximum width.
    pub fn with_max_width(mut self, max_width: UiPx) -> Self {
        self.max_width = normalized_column_width(max_width).max(self.min_width);
        self
    }

    /// Applies resize enablement.
    pub const fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Resolves this column's width against committed sizing state.
    pub fn resolved_width(&self, sizing: &TableColumnSizing) -> UiPx {
        let width = sizing.width(&self.id).unwrap_or(self.width);
        clamp_column_width(width, self.min_width, self.max_width)
    }
}

/// Caller-owned committed column sizing map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnSizing {
    widths: BTreeMap<TableColumnId, UiPx>,
}

impl TableColumnSizing {
    /// Creates an empty sizing map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a sizing map from explicit column widths.
    pub fn from_widths(widths: impl IntoIterator<Item = (impl Into<TableColumnId>, UiPx)>) -> Self {
        let mut sizing = Self::default();
        for (column, width) in widths {
            sizing = sizing.with_width(column, width);
        }
        sizing
    }

    /// Returns the committed width for a column, if present.
    pub fn width(&self, column: &TableColumnId) -> Option<UiPx> {
        self.widths.get(column).copied()
    }

    /// Returns the committed sizing map.
    pub fn widths(&self) -> &BTreeMap<TableColumnId, UiPx> {
        &self.widths
    }

    /// Returns whether no committed widths exist.
    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    /// Inserts or updates a committed column width.
    pub fn with_width(mut self, column: impl Into<TableColumnId>, width: UiPx) -> Self {
        self.widths
            .insert(column.into(), normalized_column_width(width));
        self
    }

    /// Removes a committed column width.
    pub fn without_width(mut self, column: impl Into<TableColumnId>) -> Self {
        self.widths.remove(&column.into());
        self
    }
}

fn normalized_column_width(width: UiPx) -> UiPx {
    let raw = width.as_f32();
    if raw.is_finite() {
        ui_px(raw.max(0.0))
    } else {
        UiPx::ZERO
    }
}

fn clamp_column_width(width: UiPx, min_width: UiPx, max_width: UiPx) -> UiPx {
    let min_width = normalized_column_width(min_width);
    let max_width = normalized_column_width(max_width).max(min_width);
    normalized_column_width(width).max(min_width).min(max_width)
}

/// Resolved table column lane for pinning-aware renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableColumnRegion {
    /// Columns pinned to the left side.
    Left,
    /// Unpinned center columns.
    Center,
    /// Columns pinned to the right side.
    Right,
}

impl TableColumnRegion {
    /// All column regions in render order.
    pub const ALL: [Self; 3] = [Self::Left, Self::Center, Self::Right];

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Caller-owned pinned column state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableColumnPinning {
    left: Vec<TableColumnId>,
    right: Vec<TableColumnId>,
}

impl TableColumnPinning {
    /// Creates an empty pinning state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies left-pinned column ids.
    pub fn pinned_left(
        mut self,
        columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.left = unique_column_ids(columns);
        let left = self.left.iter().cloned().collect::<BTreeSet<_>>();
        self.right.retain(|column| !left.contains(column));
        self
    }

    /// Applies right-pinned column ids.
    pub fn pinned_right(
        mut self,
        columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.right = unique_column_ids(columns);
        let right = self.right.iter().cloned().collect::<BTreeSet<_>>();
        self.left.retain(|column| !right.contains(column));
        self
    }

    /// Returns left-pinned column ids.
    pub fn left(&self) -> &[TableColumnId] {
        &self.left
    }

    /// Returns right-pinned column ids.
    pub fn right(&self) -> &[TableColumnId] {
        &self.right
    }

    /// Returns true when no columns are pinned.
    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}

/// Resolved visible columns split into render regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnRegions {
    left: Vec<TableColumn>,
    center: Vec<TableColumn>,
    right: Vec<TableColumn>,
}

impl TableColumnRegions {
    fn from_visible_columns(
        visible_columns: impl IntoIterator<Item = TableColumn>,
        pinning: &TableColumnPinning,
    ) -> Self {
        let left = pinning.left().iter().cloned().collect::<BTreeSet<_>>();
        let right = pinning.right().iter().cloned().collect::<BTreeSet<_>>();
        let mut regions = Self::default();

        for column in visible_columns {
            if left.contains(column.id()) {
                regions.left.push(column);
            } else if right.contains(column.id()) {
                regions.right.push(column);
            } else {
                regions.center.push(column);
            }
        }

        regions
    }

    /// Returns visible left-pinned columns.
    pub fn left(&self) -> &[TableColumn] {
        &self.left
    }

    /// Returns visible unpinned center columns.
    pub fn center(&self) -> &[TableColumn] {
        &self.center
    }

    /// Returns visible right-pinned columns.
    pub fn right(&self) -> &[TableColumn] {
        &self.right
    }

    /// Returns visible columns for a region.
    pub fn region(&self, region: TableColumnRegion) -> &[TableColumn] {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns the total number of visible columns across all regions.
    pub fn len(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Returns true when all regions are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn flattened(&self) -> Vec<TableColumn> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
            .cloned()
            .collect()
    }
}

/// Resolved sizing metadata for one visible table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedColumnSizing {
    column_id: TableColumnId,
    region: TableColumnRegion,
    width: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    start: UiPx,
    after: UiPx,
    resizable: bool,
}

impl TableResolvedColumnSizing {
    fn new(
        column: &TableColumn,
        region: TableColumnRegion,
        width: UiPx,
        start: UiPx,
        after: UiPx,
    ) -> Self {
        Self {
            column_id: column.id().clone(),
            region,
            width,
            min_width: column.min_width(),
            max_width: column.max_width(),
            start,
            after,
            resizable: column.resizable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved pinning region for this column.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
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

    /// Returns the offset from the end edge of this column to the region end.
    pub const fn after(&self) -> UiPx {
        self.after
    }

    /// Returns whether this column accepts resize interactions.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }
}

/// Resolved visible column sizing split into render regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableResolvedColumnSizingRegions {
    left: Vec<TableResolvedColumnSizing>,
    center: Vec<TableResolvedColumnSizing>,
    right: Vec<TableResolvedColumnSizing>,
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
}

impl TableResolvedColumnSizingRegions {
    fn from_column_regions(regions: &TableColumnRegions, sizing: &TableColumnSizing) -> Self {
        let (left, left_width) =
            resolve_column_sizing_region(TableColumnRegion::Left, regions.left(), sizing);
        let (center, center_width) =
            resolve_column_sizing_region(TableColumnRegion::Center, regions.center(), sizing);
        let (right, right_width) =
            resolve_column_sizing_region(TableColumnRegion::Right, regions.right(), sizing);

        Self {
            left,
            center,
            right,
            left_width,
            center_width,
            right_width,
            total_width: left_width + center_width + right_width,
        }
    }

    /// Returns visible left-pinned column sizing.
    pub fn left(&self) -> &[TableResolvedColumnSizing] {
        &self.left
    }

    /// Returns visible unpinned center column sizing.
    pub fn center(&self) -> &[TableResolvedColumnSizing] {
        &self.center
    }

    /// Returns visible right-pinned column sizing.
    pub fn right(&self) -> &[TableResolvedColumnSizing] {
        &self.right
    }

    /// Returns visible column sizing for a region.
    pub fn region(&self, region: TableColumnRegion) -> &[TableResolvedColumnSizing] {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns all visible column sizing in render order.
    pub fn all(&self) -> impl Iterator<Item = &TableResolvedColumnSizing> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
    }

    /// Returns the sizing metadata for a visible column.
    pub fn column(&self, column: &TableColumnId) -> Option<&TableResolvedColumnSizing> {
        self.all().find(|sizing| sizing.column_id() == column)
    }

    /// Returns the total width across all visible columns.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the total width for a specific region.
    pub const fn region_width(&self, region: TableColumnRegion) -> UiPx {
        match region {
            TableColumnRegion::Left => self.left_width,
            TableColumnRegion::Center => self.center_width,
            TableColumnRegion::Right => self.right_width,
        }
    }

    /// Returns the left-pinned region width.
    pub const fn left_width(&self) -> UiPx {
        self.left_width
    }

    /// Returns the unpinned center region width.
    pub const fn center_width(&self) -> UiPx {
        self.center_width
    }

    /// Returns the right-pinned region width.
    pub const fn right_width(&self) -> UiPx {
        self.right_width
    }

    /// Returns the number of visible columns across all regions.
    pub fn len(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Returns true when no visible column sizing exists.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn resolve_column_sizing_region(
    region: TableColumnRegion,
    columns: &[TableColumn],
    sizing: &TableColumnSizing,
) -> (Vec<TableResolvedColumnSizing>, UiPx) {
    let widths = columns
        .iter()
        .map(|column| (column, column.resolved_width(sizing)))
        .collect::<Vec<_>>();
    let total_width = widths
        .iter()
        .fold(UiPx::ZERO, |total, (_, width)| total + *width);
    let mut start = UiPx::ZERO;
    let mut resolved = Vec::with_capacity(widths.len());

    for (column, width) in widths {
        let after = total_width - start - width;
        resolved.push(TableResolvedColumnSizing::new(
            column, region, width, start, after,
        ));
        start = start + width;
    }

    (resolved, total_width)
}

/// Renderer-neutral row descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
}

impl TableRow {
    /// Creates a row with a stable identity.
    pub fn new(id: impl Into<TableRowId>) -> Self {
        Self {
            id: id.into(),
            cells: BTreeMap::new(),
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
}

/// Sort direction for a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSortDirection {
    /// Sort from low to high.
    Ascending,
    /// Sort from high to low.
    Descending,
}

impl TableSortDirection {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// Sort specification for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSort {
    column: TableColumnId,
    direction: TableSortDirection,
}

impl TableSort {
    /// Creates a sort specification.
    pub fn new(column: impl Into<TableColumnId>, direction: TableSortDirection) -> Self {
        Self {
            column: column.into(),
            direction,
        }
    }

    /// Creates an ascending sort specification.
    pub fn ascending(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableSortDirection::Ascending)
    }

    /// Creates a descending sort specification.
    pub fn descending(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableSortDirection::Descending)
    }

    /// Returns the sorted column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the sort direction.
    pub const fn direction(&self) -> TableSortDirection {
        self.direction
    }
}

/// Contains-filter specification for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFilter {
    column: TableColumnId,
    query: String,
}

impl TableFilter {
    /// Creates a case-insensitive contains filter.
    pub fn contains(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            query: query.into(),
        }
    }

    /// Returns the filtered column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the filter query.
    pub fn query(&self) -> &str {
        &self.query
    }

    fn matches(&self, row: &TableRow) -> bool {
        if self.query.is_empty() {
            return true;
        }

        row.cell(&self.column)
            .map(|value| {
                value
                    .filter_text()
                    .to_lowercase()
                    .contains(&self.query.to_lowercase())
            })
            .unwrap_or(false)
    }
}

/// Pagination state for a table row model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablePagination {
    page_index: usize,
    page_size: usize,
}

impl TablePagination {
    /// Creates pagination state from a page index and page size.
    pub const fn new(page_index: usize, page_size: usize) -> Self {
        Self {
            page_index,
            page_size,
        }
    }

    /// Returns pagination that keeps all rows.
    pub const fn disabled() -> Self {
        Self {
            page_index: 0,
            page_size: usize::MAX,
        }
    }

    /// Returns the zero-based page index.
    pub const fn page_index(self) -> usize {
        self.page_index
    }

    /// Returns the maximum number of rows per page.
    pub const fn page_size(self) -> usize {
        self.page_size
    }

    fn apply(self, rows: &[TableResolvedRow]) -> Vec<TableResolvedRow> {
        if self.page_size == usize::MAX {
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

/// Built-in aggregate calculation for grouped table rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAggregateKind {
    /// Count descendant leaf rows.
    Count,
    /// Sum numeric descendant cell values.
    Sum,
    /// Minimum numeric descendant cell value.
    Min,
    /// Maximum numeric descendant cell value.
    Max,
    /// Average numeric descendant cell value.
    Average,
}

impl TableAggregateKind {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Min => "min",
            Self::Max => "max",
            Self::Average => "average",
        }
    }
}

/// Aggregate specification for one table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableAggregation {
    column: TableColumnId,
    kind: TableAggregateKind,
}

impl TableAggregation {
    /// Creates an aggregate specification for a column.
    pub fn new(column: impl Into<TableColumnId>, kind: TableAggregateKind) -> Self {
        Self {
            column: column.into(),
            kind,
        }
    }

    /// Creates a descendant leaf-count aggregate.
    pub fn count(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Count)
    }

    /// Creates a numeric sum aggregate.
    pub fn sum(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Sum)
    }

    /// Creates a numeric minimum aggregate.
    pub fn min(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Min)
    }

    /// Creates a numeric maximum aggregate.
    pub fn max(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Max)
    }

    /// Creates a numeric average aggregate.
    pub fn average(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Average)
    }

    /// Returns the aggregate column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the aggregate kind.
    pub const fn kind(&self) -> TableAggregateKind {
        self.kind
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

/// Renderer-neutral input state for table row-model resolution.
#[derive(Debug, Clone)]
pub struct TableState {
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    rows: Arc<[TableRow]>,
    rows_identity: u64,
    sorting: Vec<TableSort>,
    filters: Vec<TableFilter>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    expansion: TableExpansionState,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl PartialEq for TableState {
    fn eq(&self, other: &Self) -> bool {
        self.columns == other.columns
            && self.column_order == other.column_order
            && self.column_pinning == other.column_pinning
            && self.column_sizing == other.column_sizing
            && self.rows.as_ref() == other.rows.as_ref()
            && self.sorting == other.sorting
            && self.filters == other.filters
            && self.grouping == other.grouping
            && self.aggregations == other.aggregations
            && self.expansion == other.expansion
            && self.selected_rows == other.selected_rows
            && self.pagination == other.pagination
    }
}

impl TableState {
    /// Creates table state from row descriptors.
    pub fn new(rows: impl IntoIterator<Item = TableRow>) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();

        Self {
            columns: Vec::new(),
            column_order: Vec::new(),
            column_pinning: TableColumnPinning::default(),
            column_sizing: TableColumnSizing::default(),
            rows: rows.into(),
            rows_identity: next_table_rows_identity(),
            sorting: Vec::new(),
            filters: Vec::new(),
            grouping: Vec::new(),
            aggregations: Vec::new(),
            expansion: TableExpansionState::default(),
            selected_rows: BTreeSet::new(),
            pagination: TablePagination::default(),
        }
    }

    /// Applies column descriptors.
    pub fn with_columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Applies explicit column order.
    pub fn with_column_order(
        mut self,
        column_order: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.column_order = column_order.into_iter().map(Into::into).collect();
        self
    }

    /// Applies pinned column state.
    pub fn with_column_pinning(mut self, column_pinning: TableColumnPinning) -> Self {
        self.column_pinning = column_pinning;
        self
    }

    /// Applies committed column sizing state.
    pub fn with_column_sizing(mut self, column_sizing: TableColumnSizing) -> Self {
        self.column_sizing = column_sizing;
        self
    }

    /// Applies sort specifications.
    pub fn with_sorting(mut self, sorting: impl IntoIterator<Item = TableSort>) -> Self {
        self.sorting = sorting.into_iter().collect();
        self
    }

    /// Applies filter specifications.
    pub fn with_filters(mut self, filters: impl IntoIterator<Item = TableFilter>) -> Self {
        self.filters = filters.into_iter().collect();
        self
    }

    /// Applies grouping column ids in outer-to-inner order.
    pub fn with_grouping(
        mut self,
        grouping: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        let mut seen = BTreeSet::new();
        self.grouping = grouping
            .into_iter()
            .map(Into::into)
            .filter(|column| seen.insert(column.clone()))
            .collect();
        self
    }

    /// Applies aggregate specifications keyed by column id.
    pub fn with_aggregations(
        mut self,
        aggregations: impl IntoIterator<Item = TableAggregation>,
    ) -> Self {
        let mut aggregations_by_column = BTreeMap::new();
        for aggregation in aggregations {
            aggregations_by_column.insert(aggregation.column().clone(), aggregation);
        }
        self.aggregations = aggregations_by_column.into_values().collect();
        self
    }

    /// Applies explicit expanded group row ids.
    pub fn with_expanded_rows(
        mut self,
        expanded_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.expansion = TableExpansionState::rows(expanded_rows);
        self
    }

    /// Applies the expansion mode where every group row is expanded.
    pub fn with_all_rows_expanded(mut self) -> Self {
        self.expansion = TableExpansionState::All;
        self
    }

    /// Applies selected row ids.
    pub fn with_selected_rows(
        mut self,
        selected_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.selected_rows = selected_rows.into_iter().map(Into::into).collect();
        self
    }

    /// Applies pagination state.
    pub const fn with_pagination(mut self, pagination: TablePagination) -> Self {
        self.pagination = pagination;
        self
    }

    /// Returns configured columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns source rows.
    pub fn rows(&self) -> &[TableRow] {
        self.rows.as_ref()
    }

    /// Returns sort specifications.
    pub fn sorting(&self) -> &[TableSort] {
        &self.sorting
    }

    /// Returns filter specifications.
    pub fn filters(&self) -> &[TableFilter] {
        &self.filters
    }

    /// Returns grouping column ids in outer-to-inner order.
    pub fn grouping(&self) -> &[TableColumnId] {
        &self.grouping
    }

    /// Returns aggregate specifications keyed by column id.
    pub fn aggregations(&self) -> &[TableAggregation] {
        &self.aggregations
    }

    /// Returns pinned column state.
    pub const fn column_pinning(&self) -> &TableColumnPinning {
        &self.column_pinning
    }

    /// Returns committed column sizing state.
    pub const fn column_sizing(&self) -> &TableColumnSizing {
        &self.column_sizing
    }

    /// Returns caller-owned row expansion state.
    pub const fn expansion(&self) -> &TableExpansionState {
        &self.expansion
    }

    /// Returns selected row ids.
    pub const fn selected_rows(&self) -> &BTreeSet<TableRowId> {
        &self.selected_rows
    }

    /// Returns pagination state.
    pub const fn pagination(&self) -> TablePagination {
        self.pagination
    }

    /// Returns a cheap identity key for runtime row-model caches.
    ///
    /// The key is conservative: cloned states share the row identity, while newly
    /// constructed states get a new identity even when their row contents match.
    pub fn cache_key(&self) -> TableStateCacheKey {
        TableStateCacheKey {
            rows_identity: self.rows_identity,
            row_count: self.rows.len(),
            columns: self.columns.clone(),
            column_order: self.column_order.clone(),
            column_pinning: self.column_pinning.clone(),
            column_sizing: self.column_sizing.clone(),
            sorting: self.sorting.clone(),
            filters: self.filters.clone(),
            grouping: self.grouping.clone(),
            aggregations: self.aggregations.clone(),
            expansion: self.expansion.clone(),
            selected_rows: self.selected_rows.clone(),
            pagination: self.pagination,
        }
    }

    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> Vec<TableColumn> {
        self.visible_column_regions().flattened()
    }

    /// Returns visible columns split into pinned regions.
    pub fn visible_column_regions(&self) -> TableColumnRegions {
        TableColumnRegions::from_visible_columns(
            self.ordered_visible_columns(),
            &self.column_pinning,
        )
    }

    fn ordered_visible_columns(&self) -> Vec<TableColumn> {
        if self.column_order.is_empty() {
            return self
                .columns
                .iter()
                .filter(|column| column.visible())
                .cloned()
                .collect();
        }

        let columns_by_id: BTreeMap<_, _> = self
            .columns
            .iter()
            .map(|column| (column.id().clone(), column.clone()))
            .collect();

        self.column_order
            .iter()
            .filter_map(|id| columns_by_id.get(id))
            .filter(|column| column.visible())
            .cloned()
            .collect()
    }

    /// Resolves row models from the input state.
    pub fn resolve(&self) -> TableResolvedState {
        let mut duplicate_row_ids = BTreeSet::new();
        let mut seen_row_ids = BTreeSet::new();
        let core_rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(source_index, row)| {
                if !seen_row_ids.insert(row.id().clone()) {
                    duplicate_row_ids.insert(row.id().clone());
                }
                TableResolvedRow::from_row(row, source_index, self.selected_rows.contains(row.id()))
            })
            .collect();

        let core_model = TableRowModel::new(TableRowModelStage::Core, core_rows);

        let filtered_rows: Vec<_> = core_model
            .rows()
            .iter()
            .filter(|row| {
                row.source()
                    .is_some_and(|source| self.filters.iter().all(|filter| filter.matches(source)))
            })
            .cloned()
            .collect();
        let filtered_model = TableRowModel::new(TableRowModelStage::Filtered, filtered_rows);

        let grouped_nodes = self.group_nodes(filtered_model.rows());
        let grouped_rows = flatten_nodes(&grouped_nodes);
        let grouped_model = TableRowModel::new(TableRowModelStage::Grouped, grouped_rows);

        let sorted_nodes = self.sort_nodes(grouped_nodes);
        let sorted_rows = flatten_nodes(&sorted_nodes);
        let sorted_model = TableRowModel::new(TableRowModelStage::Sorted, sorted_rows);

        let expanded_rows = self.expand_nodes(&sorted_nodes);
        let expanded_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Expanded,
            expanded_rows,
            sorted_model.rows().to_vec(),
        );

        let paginated_model = TableRowModel::new(
            TableRowModelStage::Paginated,
            self.pagination.apply(expanded_model.rows()),
        );
        let final_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Final,
            paginated_model.rows().to_vec(),
            expanded_model.rows_by_id().values().cloned(),
        );

        let visible_column_regions = self.visible_column_regions();
        let visible_column_sizing = TableResolvedColumnSizingRegions::from_column_regions(
            &visible_column_regions,
            &self.column_sizing,
        );

        TableResolvedState {
            visible_columns: visible_column_regions.flattened(),
            visible_column_regions,
            visible_column_sizing,
            duplicate_row_ids: duplicate_row_ids.into_iter().collect(),
            core_model,
            filtered_model,
            grouped_model,
            sorted_model,
            expanded_model,
            paginated_model,
            final_model,
        }
    }

    fn compare_rows(&self, left: &TableResolvedRow, right: &TableResolvedRow) -> Ordering {
        if self.sorting.is_empty() {
            return Ordering::Equal;
        }

        for sort in &self.sorting {
            let left_value = left.cell(sort.column()).cloned().unwrap_or_default();
            let right_value = right.cell(sort.column()).cloned().unwrap_or_default();
            let ordering = left_value.cmp_for_sort(&right_value);
            let ordering = match sort.direction() {
                TableSortDirection::Ascending => ordering,
                TableSortDirection::Descending => ordering.reverse(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        left.id().cmp(right.id())
    }

    fn group_nodes(&self, rows: &[TableResolvedRow]) -> Vec<TableRowNode> {
        if self.grouping.is_empty() {
            return rows
                .iter()
                .cloned()
                .map(TableRowNode::leaf)
                .collect::<Vec<_>>();
        }

        build_group_nodes(rows, &self.grouping, &self.aggregations, 0, None, None)
    }

    fn sort_nodes(&self, mut nodes: Vec<TableRowNode>) -> Vec<TableRowNode> {
        for node in &mut nodes {
            node.children = self.sort_nodes(std::mem::take(&mut node.children));
        }

        if !self.sorting.is_empty() {
            nodes.sort_by(|left, right| self.compare_rows(&left.row, &right.row));
        }

        nodes
    }

    fn expand_nodes(&self, nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
        let mut rows = Vec::new();
        for node in nodes {
            push_expanded_rows(node, &self.expansion, &mut rows);
        }
        rows
    }
}

/// Cheap invalidation key for runtime caches of resolved table row models.
#[derive(Debug, Clone, PartialEq)]
pub struct TableStateCacheKey {
    rows_identity: u64,
    row_count: usize,
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    sorting: Vec<TableSort>,
    filters: Vec<TableFilter>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    expansion: TableExpansionState,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl TableStateCacheKey {
    /// Returns the opaque identity assigned to this state's row storage.
    pub const fn rows_identity(&self) -> u64 {
        self.rows_identity
    }

    /// Returns the number of source rows covered by this cache key.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }
}

fn next_table_rows_identity() -> u64 {
    NEXT_TABLE_ROWS_IDENTITY.fetch_add(1, AtomicOrdering::Relaxed)
}

fn unique_column_ids(
    columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
) -> Vec<TableColumnId> {
    let mut seen = BTreeSet::new();
    columns
        .into_iter()
        .map(Into::into)
        .filter(|column| seen.insert(column.clone()))
        .collect()
}

/// Metadata for a grouped table row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGroupRow {
    grouping_column: TableColumnId,
    grouping_value: TableCellValue,
    depth: usize,
    parent_id: Option<TableRowId>,
    first_leaf_row_id: TableRowId,
    leaf_row_count: usize,
}

impl TableGroupRow {
    fn new(
        grouping_column: TableColumnId,
        grouping_value: TableCellValue,
        depth: usize,
        parent_id: Option<TableRowId>,
        first_leaf_row_id: TableRowId,
        leaf_row_count: usize,
    ) -> Self {
        Self {
            grouping_column,
            grouping_value,
            depth,
            parent_id,
            first_leaf_row_id,
            leaf_row_count,
        }
    }

    /// Returns the grouped column identity.
    pub const fn grouping_column(&self) -> &TableColumnId {
        &self.grouping_column
    }

    /// Returns the grouped value.
    pub const fn grouping_value(&self) -> &TableCellValue {
        &self.grouping_value
    }

    /// Returns this group row's depth in the grouped tree.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns the first descendant leaf row id.
    pub const fn first_leaf_row_id(&self) -> &TableRowId {
        &self.first_leaf_row_id
    }

    /// Returns the descendant leaf row count.
    pub const fn leaf_row_count(&self) -> usize {
        self.leaf_row_count
    }
}

/// Resolved row kind for Open GPUI table row models.
#[derive(Debug, Clone, PartialEq)]
pub enum TableResolvedRowKind {
    /// A row backed by one source data row.
    Leaf,
    /// A synthetic grouped row.
    Group(TableGroupRow),
}

/// A resolved row that carries source identity and derived metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedRow {
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    source: Option<TableRow>,
    source_index: Option<usize>,
    selected: bool,
    kind: TableResolvedRowKind,
    depth: usize,
    parent_id: Option<TableRowId>,
}

impl TableResolvedRow {
    fn from_row(row: &TableRow, source_index: usize, selected: bool) -> Self {
        Self {
            id: row.id().clone(),
            cells: row.cells().clone(),
            source: Some(row.clone()),
            source_index: Some(source_index),
            selected,
            kind: TableResolvedRowKind::Leaf,
            depth: 0,
            parent_id: None,
        }
    }

    fn from_group(
        id: TableRowId,
        group: TableGroupRow,
        aggregate_cells: BTreeMap<TableColumnId, TableCellValue>,
    ) -> Self {
        let mut cells = aggregate_cells;
        cells.insert(
            group.grouping_column().clone(),
            group.grouping_value().clone(),
        );

        Self {
            id,
            cells,
            source: None,
            source_index: None,
            selected: false,
            depth: group.depth(),
            parent_id: group.parent_id().cloned(),
            kind: TableResolvedRowKind::Group(group),
        }
    }

    fn with_parent(mut self, parent_id: TableRowId, depth: usize) -> Self {
        self.parent_id = Some(parent_id);
        self.depth = depth;
        self
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns the resolved row kind.
    pub const fn kind(&self) -> &TableResolvedRowKind {
        &self.kind
    }

    /// Returns true when this is a grouped row.
    pub const fn is_group(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Group(_))
    }

    /// Returns true when this is a leaf source row.
    pub const fn is_leaf(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Leaf)
    }

    /// Returns grouped row metadata when this row is a group row.
    pub const fn group(&self) -> Option<&TableGroupRow> {
        match &self.kind {
            TableResolvedRowKind::Group(group) => Some(group),
            TableResolvedRowKind::Leaf => None,
        }
    }

    /// Returns the original row descriptor for leaf rows.
    pub const fn source(&self) -> Option<&TableRow> {
        self.source.as_ref()
    }

    /// Returns all resolved cells keyed by column identity.
    pub const fn cells(&self) -> &BTreeMap<TableColumnId, TableCellValue> {
        &self.cells
    }

    /// Returns a resolved cell value for the given column.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellValue> {
        self.cells.get(column)
    }

    /// Returns the original source index before row-model transforms for leaf rows.
    pub const fn source_index(&self) -> Option<usize> {
        self.source_index
    }

    /// Returns this row's depth in a grouped row model.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns whether this row id is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Resolved rows for one row-model stage.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowModel {
    stage: TableRowModelStage,
    rows: Vec<TableResolvedRow>,
    rows_by_id: BTreeMap<TableRowId, TableResolvedRow>,
}

impl TableRowModel {
    /// Creates a row model from rows at one stage.
    pub fn new(stage: TableRowModelStage, rows: impl Into<Vec<TableResolvedRow>>) -> Self {
        let rows = rows.into();
        Self::new_with_lookup(stage, rows.clone(), rows)
    }

    fn new_with_lookup(
        stage: TableRowModelStage,
        rows: impl Into<Vec<TableResolvedRow>>,
        lookup_rows: impl IntoIterator<Item = TableResolvedRow>,
    ) -> Self {
        let rows = rows.into();
        let rows_by_id = lookup_rows
            .into_iter()
            .map(|row| (row.id().clone(), row))
            .collect();

        Self {
            stage,
            rows,
            rows_by_id,
        }
    }

    /// Returns this model's stage.
    pub const fn stage(&self) -> TableRowModelStage {
        self.stage
    }

    /// Returns rows in model order.
    pub fn rows(&self) -> &[TableResolvedRow] {
        &self.rows
    }

    /// Returns the row lookup for this model.
    pub const fn rows_by_id(&self) -> &BTreeMap<TableRowId, TableResolvedRow> {
        &self.rows_by_id
    }

    /// Returns a row by stable id.
    pub fn row(&self, id: &TableRowId) -> Option<&TableResolvedRow> {
        self.rows_by_id.get(id)
    }

    /// Returns the number of selected rows in this model.
    pub fn selected_count(&self) -> usize {
        self.rows.iter().filter(|row| row.selected()).count()
    }
}

/// Resolved table row models and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedState {
    visible_columns: Vec<TableColumn>,
    visible_column_regions: TableColumnRegions,
    visible_column_sizing: TableResolvedColumnSizingRegions,
    duplicate_row_ids: Vec<TableRowId>,
    core_model: TableRowModel,
    filtered_model: TableRowModel,
    grouped_model: TableRowModel,
    sorted_model: TableRowModel,
    expanded_model: TableRowModel,
    paginated_model: TableRowModel,
    final_model: TableRowModel,
}

impl TableResolvedState {
    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> &[TableColumn] {
        &self.visible_columns
    }

    /// Returns visible columns split into pinned regions.
    pub const fn visible_column_regions(&self) -> &TableColumnRegions {
        &self.visible_column_regions
    }

    /// Returns resolved visible column sizing split into pinned regions.
    pub const fn visible_column_sizing(&self) -> &TableResolvedColumnSizingRegions {
        &self.visible_column_sizing
    }

    /// Returns duplicate source row ids detected during resolution.
    pub fn duplicate_row_ids(&self) -> &[TableRowId] {
        &self.duplicate_row_ids
    }

    /// Returns the core row model.
    pub const fn core_model(&self) -> &TableRowModel {
        &self.core_model
    }

    /// Returns the filtered row model.
    pub const fn filtered_model(&self) -> &TableRowModel {
        &self.filtered_model
    }

    /// Returns the grouped row model.
    pub const fn grouped_model(&self) -> &TableRowModel {
        &self.grouped_model
    }

    /// Returns the sorted row model.
    pub const fn sorted_model(&self) -> &TableRowModel {
        &self.sorted_model
    }

    /// Returns the expanded row model.
    pub const fn expanded_model(&self) -> &TableRowModel {
        &self.expanded_model
    }

    /// Returns the paginated row model.
    pub const fn paginated_model(&self) -> &TableRowModel {
        &self.paginated_model
    }

    /// Returns the final row model consumed by renderers.
    pub const fn final_model(&self) -> &TableRowModel {
        &self.final_model
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableRowNode {
    row: TableResolvedRow,
    children: Vec<TableRowNode>,
}

impl TableRowNode {
    fn leaf(row: TableResolvedRow) -> Self {
        Self {
            row,
            children: Vec::new(),
        }
    }
}

fn flatten_nodes(nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push(node.row.clone());
        rows.extend(flatten_nodes(&node.children));
    }
    rows
}

fn build_group_nodes(
    rows: &[TableResolvedRow],
    grouping: &[TableColumnId],
    aggregations: &[TableAggregation],
    depth: usize,
    parent_group_id: Option<TableRowId>,
    inherited_parent_id: Option<TableRowId>,
) -> Vec<TableRowNode> {
    if grouping.is_empty() {
        return rows
            .iter()
            .cloned()
            .map(|row| {
                let row = match inherited_parent_id.as_ref() {
                    Some(parent_id) => row.with_parent(parent_id.clone(), depth),
                    None => row,
                };
                TableRowNode::leaf(row)
            })
            .collect();
    }

    let grouping_column = grouping[0].clone();
    let mut buckets: Vec<(String, TableCellValue, Vec<TableResolvedRow>)> = Vec::new();
    let mut bucket_index_by_key = BTreeMap::new();

    for row in rows {
        let value = row.cell(&grouping_column).cloned().unwrap_or_default();
        let key = value.filter_text();
        let index = match bucket_index_by_key.get(&key).copied() {
            Some(index) => index,
            None => {
                let index = buckets.len();
                bucket_index_by_key.insert(key.clone(), index);
                buckets.push((key.clone(), value.clone(), Vec::new()));
                index
            }
        };
        buckets[index].2.push(row.clone());
    }

    let mut nodes = Vec::new();
    for (value_text, value, bucket_rows) in buckets {
        let group_id = build_group_row_id(parent_group_id.as_ref(), &grouping_column, &value_text);
        let first_leaf_row_id = bucket_rows
            .first()
            .map(|row| row.id().clone())
            .unwrap_or_else(|| group_id.clone());
        let leaf_row_count = bucket_rows.len();
        let parent_id = inherited_parent_id.clone();
        let group = TableGroupRow::new(
            grouping_column.clone(),
            value,
            depth,
            parent_id.clone(),
            first_leaf_row_id,
            leaf_row_count,
        );
        let children = build_group_nodes(
            &bucket_rows,
            &grouping[1..],
            aggregations,
            depth + 1,
            Some(group_id.clone()),
            Some(group_id.clone()),
        );
        let aggregate_cells = resolve_aggregate_cells(&bucket_rows, aggregations);
        let row = TableResolvedRow::from_group(group_id, group, aggregate_cells);
        nodes.push(TableRowNode { row, children });
    }

    nodes
}

fn resolve_aggregate_cells(
    rows: &[TableResolvedRow],
    aggregations: &[TableAggregation],
) -> BTreeMap<TableColumnId, TableCellValue> {
    aggregations
        .iter()
        .map(|aggregation| {
            (
                aggregation.column().clone(),
                resolve_aggregate_cell(rows, aggregation),
            )
        })
        .collect()
}

fn resolve_aggregate_cell(
    rows: &[TableResolvedRow],
    aggregation: &TableAggregation,
) -> TableCellValue {
    match aggregation.kind() {
        TableAggregateKind::Count => TableCellValue::Number(rows.len() as f64),
        TableAggregateKind::Sum => {
            let mut seen_numeric = false;
            let sum = numeric_values(rows, aggregation.column()).fold(0.0, |sum, value| {
                seen_numeric = true;
                sum + value
            });

            if seen_numeric {
                TableCellValue::Number(sum)
            } else {
                TableCellValue::Empty
            }
        }
        TableAggregateKind::Min => numeric_values(rows, aggregation.column())
            .min_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Max => numeric_values(rows, aggregation.column())
            .max_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Average => {
            let mut count = 0_usize;
            let sum = numeric_values(rows, aggregation.column()).fold(0.0, |sum, value| {
                count += 1;
                sum + value
            });

            if count > 0 {
                TableCellValue::Number(sum / count as f64)
            } else {
                TableCellValue::Empty
            }
        }
    }
}

fn numeric_values<'a>(
    rows: &'a [TableResolvedRow],
    column: &'a TableColumnId,
) -> impl Iterator<Item = f64> + 'a {
    rows.iter().filter_map(|row| match row.cell(column) {
        Some(TableCellValue::Number(value)) => Some(*value),
        _ => None,
    })
}

fn build_group_row_id(
    parent_id: Option<&TableRowId>,
    column: &TableColumnId,
    value_text: &str,
) -> TableRowId {
    let segment = format!("{}={}", column.as_str(), value_text);
    match parent_id {
        Some(parent) => TableRowId::new(format!("{}>{segment}", parent.as_str())),
        None => TableRowId::new(format!("group:{segment}")),
    }
}

fn push_expanded_rows(
    node: &TableRowNode,
    expansion: &TableExpansionState,
    rows: &mut Vec<TableResolvedRow>,
) {
    rows.push(node.row.clone());
    if node.children.is_empty() {
        return;
    }

    if node.row.is_group() && !expansion.is_expanded(node.row.id()) {
        return;
    }

    for child in &node.children {
        push_expanded_rows(child, expansion, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-b")
                .with_cell("name", "Beta")
                .with_cell("team", "ops")
                .with_cell("score", 20_usize),
            TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "design")
                .with_cell("score", 10_usize),
            TableRow::new("row-c")
                .with_cell("name", "Gamma")
                .with_cell("team", "ops")
                .with_cell("score", 30_usize),
        ]
    }

    fn aggregate_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-1")
                .with_cell("team", "ops")
                .with_cell("name", "Alpha")
                .with_cell("score", 20_usize)
                .with_cell("low", 4_usize)
                .with_cell("high", 11_usize)
                .with_cell("duration", 2_usize)
                .with_cell("noise", "n/a"),
            TableRow::new("row-2")
                .with_cell("team", "ops")
                .with_cell("name", "Beta")
                .with_cell("score", 30_usize)
                .with_cell("low", 2_usize)
                .with_cell("high", 9_usize)
                .with_cell("duration", 4_usize)
                .with_cell("noise", "unknown"),
            TableRow::new("row-3")
                .with_cell("team", "design")
                .with_cell("name", "Gamma")
                .with_cell("score", 7_usize)
                .with_cell("low", 7_usize)
                .with_cell("high", 7_usize)
                .with_cell("duration", 10_usize)
                .with_cell("noise", "unknown"),
        ]
    }

    #[test]
    fn row_model_pipeline_names_full_and_v0_stages() {
        assert_eq!(
            TABLE_ROW_MODEL_PIPELINE.map(TableRowModelStage::as_str),
            [
                "core",
                "filtered",
                "grouped",
                "sorted",
                "expanded",
                "paginated",
                "final"
            ]
        );
        assert_eq!(
            TABLE_ROW_MODEL_V0_PIPELINE.map(TableRowModelStage::as_str),
            ["core", "filtered", "sorted", "paginated", "final"]
        );
        assert!(!TableRowModelStage::Grouped.implemented_in_v0());
        assert!(!TableRowModelStage::Expanded.implemented_in_v0());
        assert!(TableRowModelStage::Sorted.implemented_in_v0());
    }

    #[test]
    fn column_widths_resolve_from_defaults_and_committed_sizing() {
        let column = TableColumn::new("name", "Name")
            .with_width(ui_px(120.0))
            .with_min_width(ui_px(80.0))
            .with_max_width(ui_px(160.0));

        assert_eq!(column.width(), ui_px(120.0));
        assert_eq!(column.min_width(), ui_px(80.0));
        assert_eq!(column.max_width(), ui_px(160.0));
        assert!(column.resizable());
        assert_eq!(
            column.resolved_width(&TableColumnSizing::new()),
            ui_px(120.0),
            "without committed sizing, the preferred width is used"
        );

        let undersized = TableColumnSizing::new().with_width("name", ui_px(40.0));
        assert_eq!(
            column.resolved_width(&undersized),
            ui_px(80.0),
            "committed widths are clamped to the column minimum"
        );

        let oversized = TableColumnSizing::new().with_width("name", ui_px(220.0));
        assert_eq!(
            column.resolved_width(&oversized),
            ui_px(160.0),
            "committed widths are clamped to the column maximum"
        );

        let unrelated = TableColumnSizing::new().with_width("team", ui_px(140.0));
        assert_eq!(
            column.resolved_width(&unrelated),
            ui_px(120.0),
            "unknown committed sizing ids do not affect this column"
        );
    }

    #[test]
    fn sizing_state_keeps_unknown_ids_without_changing_visible_columns() {
        let state = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
            ])
            .with_column_sizing(TableColumnSizing::from_widths([
                ("team", ui_px(320.0)),
                ("missing", ui_px(480.0)),
            ]));

        let visible_columns = state.visible_columns();
        assert_eq!(
            visible_columns
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            visible_columns[0].resolved_width(state.column_sizing()),
            TABLE_DEFAULT_COLUMN_WIDTH,
            "hidden and unknown sizing entries do not contribute visible widths"
        );
        assert_eq!(
            state.column_sizing().width(&TableColumnId::new("missing")),
            Some(ui_px(480.0)),
            "unknown ids remain caller-owned state instead of being silently pruned"
        );
    }

    #[test]
    fn resolved_column_sizing_tracks_region_offsets_and_totals() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
                TableColumn::new("score", "Score")
                    .with_width(ui_px(80.0))
                    .with_min_width(ui_px(70.0))
                    .with_max_width(ui_px(90.0)),
                TableColumn::new("status", "Status")
                    .with_width(ui_px(60.0))
                    .with_resizable(false),
            ])
            .with_column_order(["status", "score", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score"])
                    .pinned_right(["status"]),
            )
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(95.0)))
            .resolve();
        let sizing = resolved.visible_column_sizing();

        assert_eq!(sizing.left_width(), ui_px(190.0));
        assert_eq!(sizing.center_width(), ui_px(120.0));
        assert_eq!(sizing.right_width(), ui_px(60.0));
        assert_eq!(sizing.total_width(), ui_px(370.0));
        assert_eq!(sizing.region_width(TableColumnRegion::Left), ui_px(190.0));
        assert_eq!(
            sizing
                .left()
                .iter()
                .map(|column| {
                    (
                        column.column_id().as_str(),
                        column.width(),
                        column.start(),
                        column.after(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("score", ui_px(90.0), ui_px(0.0), ui_px(100.0)),
                ("name", ui_px(100.0), ui_px(90.0), ui_px(0.0)),
            ],
            "left region offsets follow resolved visible order and clamp committed widths"
        );

        let status = sizing
            .column(&TableColumnId::new("status"))
            .expect("status sizing should resolve");
        assert_eq!(status.region(), TableColumnRegion::Right);
        assert_eq!(status.width(), ui_px(60.0));
        assert_eq!(status.start(), ui_px(0.0));
        assert_eq!(status.after(), ui_px(0.0));
        assert!(!status.resizable());
    }

    #[test]
    fn resolved_column_sizing_is_stable_across_row_model_changes() {
        let base = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
                TableColumn::new("score", "Score").with_width(ui_px(80.0)),
            ])
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(96.0)));

        let base_sizing = base.resolve().visible_column_sizing().clone();
        let changed_rows = base
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["row-a"])
            .with_pagination(TablePagination::new(0, 1))
            .resolve()
            .visible_column_sizing()
            .clone();

        assert_eq!(base_sizing, changed_rows);
    }

    #[test]
    fn stable_row_ids_survive_filtering_sorting_and_pagination() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::new(0, 1))
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-c"]
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c", "row-b"]
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c"]
        );
        assert!(
            resolved
                .core_model()
                .row(&TableRowId::new("row-b"))
                .is_some()
        );
    }

    #[test]
    fn row_lookup_does_not_depend_on_numeric_index_positions() {
        let resolved = TableState::new(sample_rows())
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let row_c = resolved
            .core_model()
            .row(&TableRowId::new("row-c"))
            .expect("row-c should remain addressable by id");

        assert_eq!(row_c.source_index(), Some(2));
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-a", "row-b", "row-c"]
        );
    }

    #[test]
    fn selection_follows_row_ids_after_filtering_and_sorting() {
        let resolved = TableState::new(sample_rows())
            .with_selected_rows(["row-c"])
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let selected = resolved
            .final_model()
            .row(&TableRowId::new("row-c"))
            .expect("selected row should still be present");

        assert!(selected.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn grouped_row_model_creates_stable_group_rows() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .grouped_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            [
                "group:team=ops",
                "row-b",
                "row-c",
                "group:team=design",
                "row-a"
            ]
        );

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group row should be addressable by id");
        let ops_group = ops.group().expect("ops row should be a group row");

        assert_eq!(ops_group.grouping_column().as_str(), "team");
        assert_eq!(ops_group.grouping_value().filter_text(), "ops");
        assert_eq!(ops_group.depth(), 0);
        assert_eq!(ops_group.parent_id(), None);
        assert_eq!(ops_group.first_leaf_row_id().as_str(), "row-b");
        assert_eq!(ops_group.leaf_row_count(), 2);
        assert!(ops.is_group());
    }

    #[test]
    fn collapsed_groups_hide_descendants_but_preserve_lookup() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .expanded_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "group:team=design"]
        );
        assert!(
            resolved
                .expanded_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "collapsed descendants should remain addressable in lookup metadata"
        );
    }

    #[test]
    fn expanded_groups_show_descendants_with_parent_depth_and_selection() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_expanded_rows(["group:team=ops"])
            .with_selected_rows(["row-c"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b", "row-c", "group:team=design"]
        );

        let row_c = resolved
            .final_model()
            .rows()
            .iter()
            .find(|row| row.id().as_str() == "row-c")
            .expect("expanded descendant should be visible");

        assert_eq!(row_c.depth(), 1);
        assert_eq!(
            row_c.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert!(row_c.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn multi_column_grouping_creates_nested_group_paths() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team", "score"])
            .resolve();

        let nested = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops>score=20"))
            .expect("nested score group should use the parent path");
        let group = nested.group().expect("nested row should be grouped");

        assert_eq!(group.depth(), 1);
        assert_eq!(
            group.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert_eq!(group.leaf_row_count(), 1);
    }

    #[test]
    fn pagination_applies_after_expansion() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_all_rows_expanded()
            .with_pagination(TablePagination::new(0, 2))
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "final lookup keeps expanded-but-not-paginated rows addressable"
        );
    }

    #[test]
    fn aggregate_kind_labels_are_stable() {
        assert_eq!(TableAggregateKind::Count.as_str(), "count");
        assert_eq!(TableAggregateKind::Sum.as_str(), "sum");
        assert_eq!(TableAggregateKind::Min.as_str(), "min");
        assert_eq!(TableAggregateKind::Max.as_str(), "max");
        assert_eq!(TableAggregateKind::Average.as_str(), "average");
    }

    #[test]
    fn grouped_rows_expose_builtin_aggregate_cells() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::sum("score"),
                TableAggregation::min("low"),
                TableAggregation::max("high"),
                TableAggregation::average("duration"),
                TableAggregation::sum("noise"),
            ])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("name")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("score")),
            Some(&TableCellValue::Number(50.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("low")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("high")),
            Some(&TableCellValue::Number(11.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("duration")),
            Some(&TableCellValue::Number(3.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("noise")),
            Some(&TableCellValue::Empty)
        );
    }

    #[test]
    fn grouping_value_overrides_aggregate_on_grouping_column() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([TableAggregation::count("team")])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("team")),
            Some(&TableCellValue::Text("ops".to_string()))
        );
    }

    #[test]
    fn visible_columns_respect_explicit_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .with_column_order(["score", "team", "name"])
            .resolve();

        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name"]
        );
    }

    #[test]
    fn pinned_columns_split_visible_regions_after_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
                TableColumn::new("owner", "Owner"),
                TableColumn::new("status", "Status"),
            ])
            .with_column_order(["status", "score", "owner", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score", "missing"])
                    .pinned_right(["status"]),
            )
            .resolve();
        let regions = resolved.visible_column_regions();

        assert_eq!(TableColumnRegion::Left.as_str(), "left");
        assert_eq!(TableColumnRegion::Center.as_str(), "center");
        assert_eq!(TableColumnRegion::Right.as_str(), "right");
        assert_eq!(
            regions
                .left()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name"],
            "pinned left columns preserve resolved visible order"
        );
        assert_eq!(
            regions
                .center()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["owner"],
            "unknown and invisible pinned ids are ignored"
        );
        assert_eq!(
            regions
                .right()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["status"]
        );
        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name", "owner", "status"]
        );
    }

    #[test]
    fn column_pinning_moves_columns_between_regions_without_duplicates() {
        let pinning = TableColumnPinning::new()
            .pinned_left(["name", "score", "name"])
            .pinned_right(["score", "status", "score"]);

        assert_eq!(
            pinning
                .left()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            pinning
                .right()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["score", "status"]
        );
        assert!(!pinning.is_empty());
    }

    #[test]
    fn duplicate_row_ids_are_reported_without_panicking() {
        let resolved = TableState::new([
            TableRow::new("row-a").with_cell("name", "A"),
            TableRow::new("row-a").with_cell("name", "A duplicate"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .duplicate_row_ids()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["row-a"]
        );
    }

    #[test]
    fn cache_key_reuses_row_identity_for_clones_and_invalidates_state_changes() {
        let base = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);
        let cloned = base.clone();
        let sorted = base.clone().with_sorting([TableSort::descending("score")]);
        let aggregated = base
            .clone()
            .with_aggregations([TableAggregation::sum("score")]);
        let pinned = base.clone().with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["score"]),
        );
        let sized = base
            .clone()
            .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(180.0)));
        let rebuilt = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);

        assert_eq!(base, cloned);
        assert_eq!(base.cache_key(), cloned.cache_key());
        assert_eq!(
            base.cache_key().rows_identity(),
            cloned.cache_key().rows_identity()
        );

        assert_ne!(base.cache_key(), sorted.cache_key());
        assert_ne!(base.cache_key(), aggregated.cache_key());
        assert_ne!(base.cache_key(), pinned.cache_key());
        assert_ne!(base.cache_key(), sized.cache_key());
        assert_eq!(base, rebuilt);
        assert_ne!(
            base.cache_key().rows_identity(),
            rebuilt.cache_key().rows_identity()
        );
        assert_ne!(base.cache_key(), rebuilt.cache_key());
    }
}
