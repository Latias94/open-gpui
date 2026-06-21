//! Renderer-neutral table row-model contracts for Open GPUI components.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn {
    id: TableColumnId,
    label: String,
    visible: bool,
    sortable: bool,
    filterable: bool,
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

/// Row-model stage vocabulary for Open GPUI tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowModelStage {
    /// Materialized one-to-one data rows.
    Core,
    /// Filtered rows.
    Filtered,
    /// Grouped rows. Deferred in v0.
    Grouped,
    /// Sorted rows.
    Sorted,
    /// Expanded rows. Deferred in v0.
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

    /// Returns whether this stage is implemented by the v0 resolver.
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

/// First implemented row-model pipeline.
pub const TABLE_ROW_MODEL_V0_PIPELINE: [TableRowModelStage; 5] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Sorted,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

/// Renderer-neutral input state for table row-model resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct TableState {
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    rows: Vec<TableRow>,
    sorting: Vec<TableSort>,
    filters: Vec<TableFilter>,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl TableState {
    /// Creates table state from row descriptors.
    pub fn new(rows: impl IntoIterator<Item = TableRow>) -> Self {
        Self {
            columns: Vec::new(),
            column_order: Vec::new(),
            rows: rows.into_iter().collect(),
            sorting: Vec::new(),
            filters: Vec::new(),
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
        &self.rows
    }

    /// Returns sort specifications.
    pub fn sorting(&self) -> &[TableSort] {
        &self.sorting
    }

    /// Returns filter specifications.
    pub fn filters(&self) -> &[TableFilter] {
        &self.filters
    }

    /// Returns selected row ids.
    pub const fn selected_rows(&self) -> &BTreeSet<TableRowId> {
        &self.selected_rows
    }

    /// Returns pagination state.
    pub const fn pagination(&self) -> TablePagination {
        self.pagination
    }

    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> Vec<TableColumn> {
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

    /// Resolves all v0 row models from the input state.
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
                self.filters
                    .iter()
                    .all(|filter| filter.matches(row.source()))
            })
            .cloned()
            .collect();
        let filtered_model = TableRowModel::new(TableRowModelStage::Filtered, filtered_rows);

        let mut sorted_rows = filtered_model.rows().to_vec();
        sorted_rows.sort_by(|left, right| self.compare_rows(left, right));
        let sorted_model = TableRowModel::new(TableRowModelStage::Sorted, sorted_rows);

        let paginated_model = TableRowModel::new(
            TableRowModelStage::Paginated,
            self.pagination.apply(sorted_model.rows()),
        );
        let final_model = TableRowModel::new(TableRowModelStage::Final, paginated_model.rows());

        TableResolvedState {
            visible_columns: self.visible_columns(),
            duplicate_row_ids: duplicate_row_ids.into_iter().collect(),
            core_model,
            filtered_model,
            sorted_model,
            paginated_model,
            final_model,
        }
    }

    fn compare_rows(&self, left: &TableResolvedRow, right: &TableResolvedRow) -> Ordering {
        for sort in &self.sorting {
            let left_value = left
                .source()
                .cell(sort.column())
                .cloned()
                .unwrap_or_default();
            let right_value = right
                .source()
                .cell(sort.column())
                .cloned()
                .unwrap_or_default();
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
}

/// A resolved row that carries source identity and derived metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedRow {
    source: TableRow,
    source_index: usize,
    selected: bool,
}

impl TableResolvedRow {
    fn from_row(row: &TableRow, source_index: usize, selected: bool) -> Self {
        Self {
            source: row.clone(),
            source_index,
            selected,
        }
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        self.source.id()
    }

    /// Returns the original row descriptor.
    pub const fn source(&self) -> &TableRow {
        &self.source
    }

    /// Returns the original source index before row-model transforms.
    pub const fn source_index(&self) -> usize {
        self.source_index
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
        let rows_by_id = rows
            .iter()
            .map(|row| (row.id().clone(), row.clone()))
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
    duplicate_row_ids: Vec<TableRowId>,
    core_model: TableRowModel,
    filtered_model: TableRowModel,
    sorted_model: TableRowModel,
    paginated_model: TableRowModel,
    final_model: TableRowModel,
}

impl TableResolvedState {
    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> &[TableColumn] {
        &self.visible_columns
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

    /// Returns the sorted row model.
    pub const fn sorted_model(&self) -> &TableRowModel {
        &self.sorted_model
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

        assert_eq!(row_c.source_index(), 2);
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
}
