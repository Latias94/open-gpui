//! Faceting contracts and client facet derivation helpers.

use std::collections::BTreeMap;

use super::filtering::TableFilter;
use super::row_model::TableStageMode;
use super::{TableCellValue, TableColumnId, TableResolvedRow, TableRow, TableRowNode};

/// Count for one faceted table value.
#[derive(Debug, Clone)]
pub struct TableFacetValueCount {
    value: TableCellValue,
    count: usize,
}

impl PartialEq for TableFacetValueCount {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count
            && TableFacetValueKey::from_value(&self.value)
                == TableFacetValueKey::from_value(&other.value)
    }
}

impl TableFacetValueCount {
    /// Creates a count entry for one faceted value.
    pub fn new(value: impl Into<TableCellValue>, count: usize) -> Self {
        Self {
            value: value.into(),
            count,
        }
    }

    /// Returns the faceted value.
    pub const fn value(&self) -> &TableCellValue {
        &self.value
    }

    /// Returns the number of rows that produced this value.
    pub const fn count(&self) -> usize {
        self.count
    }
}

/// Numeric min/max metadata for a faceted column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableFacetRange {
    min: f64,
    max: f64,
}

impl TableFacetRange {
    /// Creates a numeric range when both bounds are finite.
    pub fn new(left: f64, right: f64) -> Option<Self> {
        if !left.is_finite() || !right.is_finite() {
            return None;
        }

        Some(if left <= right {
            Self {
                min: left,
                max: right,
            }
        } else {
            Self {
                min: right,
                max: left,
            }
        })
    }

    /// Returns the lower numeric bound.
    pub const fn min(self) -> f64 {
        self.min
    }

    /// Returns the upper numeric bound.
    pub const fn max(self) -> f64 {
        self.max
    }
}

/// Faceting metadata for one table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnFacets {
    column: TableColumnId,
    mode: TableStageMode,
    row_count: usize,
    unique_values: Vec<TableFacetValueCount>,
    numeric_range: Option<TableFacetRange>,
}

impl TableColumnFacets {
    /// Creates empty client-derived facet metadata for one column.
    pub fn new(column: impl Into<TableColumnId>) -> Self {
        Self {
            column: column.into(),
            mode: TableStageMode::Client,
            row_count: 0,
            unique_values: Vec::new(),
            numeric_range: None,
        }
    }

    /// Creates empty manual facet metadata for one column.
    pub fn manual(column: impl Into<TableColumnId>, row_count: usize) -> Self {
        Self::new(column)
            .with_mode(TableStageMode::Manual)
            .with_row_count(row_count)
    }

    pub(super) fn client(
        column: TableColumnId,
        row_count: usize,
        unique_values: Vec<TableFacetValueCount>,
        numeric_range: Option<TableFacetRange>,
    ) -> Self {
        Self {
            column,
            mode: TableStageMode::Client,
            row_count,
            unique_values,
            numeric_range,
        }
    }

    /// Returns the faceted column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns whether this facet summary was locally derived or caller supplied.
    pub const fn mode(&self) -> TableStageMode {
        self.mode
    }

    /// Returns the number of rows covered by this facet summary.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Returns unique values and their row counts.
    pub fn unique_values(&self) -> &[TableFacetValueCount] {
        &self.unique_values
    }

    /// Returns the numeric min/max range, when any finite numeric values exist.
    pub const fn numeric_range(&self) -> Option<TableFacetRange> {
        self.numeric_range
    }

    /// Applies the facet ownership mode.
    pub const fn with_mode(mut self, mode: TableStageMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies the row count covered by this facet summary.
    pub const fn with_row_count(mut self, row_count: usize) -> Self {
        self.row_count = row_count;
        self
    }

    /// Applies unique values and their row counts.
    pub fn with_unique_values(
        mut self,
        unique_values: impl IntoIterator<Item = TableFacetValueCount>,
    ) -> Self {
        self.unique_values = unique_values.into_iter().collect();
        self
    }

    /// Applies a numeric min/max range when both bounds are finite.
    pub fn with_numeric_range(mut self, min: f64, max: f64) -> Self {
        self.numeric_range = TableFacetRange::new(min, max);
        self
    }
}

/// Faceting metadata for the global filter context.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGlobalFacetSummary {
    mode: TableStageMode,
    row_count: usize,
    column_facets: Vec<TableColumnFacets>,
}

impl TableGlobalFacetSummary {
    /// Creates an empty client-derived global facet summary.
    pub fn new() -> Self {
        Self {
            mode: TableStageMode::Client,
            row_count: 0,
            column_facets: Vec::new(),
        }
    }

    pub(super) fn client(row_count: usize, column_facets: Vec<TableColumnFacets>) -> Self {
        Self {
            mode: TableStageMode::Client,
            row_count,
            column_facets,
        }
    }

    pub(super) fn manual() -> Self {
        Self {
            mode: TableStageMode::Manual,
            row_count: 0,
            column_facets: Vec::new(),
        }
    }

    /// Returns whether this summary was locally derived or caller supplied.
    pub const fn mode(&self) -> TableStageMode {
        self.mode
    }

    /// Returns the number of rows covered by the global facet basis.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Returns per-column facet summaries for globally filterable columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        &self.column_facets
    }

    /// Returns the global facet summary for one globally filterable column.
    pub fn column_facet(&self, column: &TableColumnId) -> Option<&TableColumnFacets> {
        self.column_facets
            .iter()
            .find(|facet| facet.column() == column)
    }
}

impl Default for TableGlobalFacetSummary {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TableFacetValueKey {
    Empty,
    Bool(bool),
    Number(u64),
    Text(String),
}

impl TableFacetValueKey {
    fn from_value(value: &TableCellValue) -> Self {
        match value {
            TableCellValue::Empty => Self::Empty,
            TableCellValue::Bool(value) => Self::Bool(*value),
            TableCellValue::Number(value) => Self::Number(table_facet_number_key(*value)),
            TableCellValue::Text(value) => Self::Text(value.clone()),
        }
    }
}

fn table_facet_number_key(value: f64) -> u64 {
    let normalized = if value == 0.0 { 0.0 } else { value };
    let bits = normalized.to_bits();
    if bits & (1 << 63) != 0 {
        !bits
    } else {
        bits | (1 << 63)
    }
}

pub(super) fn resolve_client_column_facets(
    column_id: TableColumnId,
    source_nodes: &[TableRowNode],
    filters: &[TableFilter],
    global_filter: Option<&str>,
    global_filterable_columns: &[TableColumnId],
    filtering_mode: TableStageMode,
) -> TableColumnFacets {
    let mut unique_values = BTreeMap::<TableFacetValueKey, TableFacetValueCount>::new();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut found_numeric = false;
    let mut row_count = 0;

    {
        let mut visit = |row: &TableResolvedRow| {
            record_column_facet_value(
                row,
                &column_id,
                &mut row_count,
                &mut unique_values,
                &mut min,
                &mut max,
                &mut found_numeric,
            );
        };

        visit_facet_rows(
            source_nodes,
            filters,
            &column_id,
            global_filter,
            global_filterable_columns,
            filtering_mode,
            &mut visit,
        );
    }

    let numeric_range = if found_numeric {
        TableFacetRange::new(min, max)
    } else {
        None
    };

    TableColumnFacets::client(
        column_id,
        row_count,
        unique_values.into_values().collect(),
        numeric_range,
    )
}

pub(super) fn resolve_client_global_column_facets(
    column_id: TableColumnId,
    source_nodes: &[TableRowNode],
) -> TableColumnFacets {
    let mut unique_values = BTreeMap::<TableFacetValueKey, TableFacetValueCount>::new();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut found_numeric = false;
    let mut row_count = 0;

    visit_source_row_nodes(source_nodes, &mut |row| {
        record_column_facet_value(
            row,
            &column_id,
            &mut row_count,
            &mut unique_values,
            &mut min,
            &mut max,
            &mut found_numeric,
        );
    });

    let numeric_range = if found_numeric {
        TableFacetRange::new(min, max)
    } else {
        None
    };

    TableColumnFacets::client(
        column_id,
        row_count,
        unique_values.into_values().collect(),
        numeric_range,
    )
}

fn record_column_facet_value(
    row: &TableResolvedRow,
    column_id: &TableColumnId,
    row_count: &mut usize,
    unique_values: &mut BTreeMap<TableFacetValueKey, TableFacetValueCount>,
    min: &mut f64,
    max: &mut f64,
    found_numeric: &mut bool,
) {
    *row_count += 1;
    let value = row.cell(column_id).cloned().unwrap_or_default();
    let key = TableFacetValueKey::from_value(&value);
    unique_values
        .entry(key)
        .and_modify(|entry| entry.count += 1)
        .or_insert_with(|| TableFacetValueCount::new(value.clone(), 1));

    if let TableCellValue::Number(number) = value {
        if number.is_finite() {
            *found_numeric = true;
            if number < *min {
                *min = number;
            }
            if number > *max {
                *max = number;
            }
        }
    }
}

fn visit_source_row_nodes(nodes: &[TableRowNode], visit: &mut impl FnMut(&TableResolvedRow)) {
    for node in nodes {
        visit(&node.row);
        visit_source_row_nodes(&node.children, visit);
    }
}

pub(super) fn visit_facet_rows(
    nodes: &[TableRowNode],
    filters: &[TableFilter],
    excluded_column: &TableColumnId,
    global_filter: Option<&str>,
    global_filterable_columns: &[TableColumnId],
    filtering_mode: TableStageMode,
    visit: &mut impl FnMut(&TableResolvedRow),
) {
    for node in nodes {
        if !filtering_mode.is_manual()
            && !row_matches_facet_filters(&node.row, filters, excluded_column)
        {
            continue;
        }
        if !filtering_mode.is_manual()
            && !resolved_row_matches_global_filter(
                &node.row,
                global_filter,
                global_filterable_columns,
            )
        {
            continue;
        }

        visit(&node.row);
        visit_facet_rows(
            &node.children,
            filters,
            excluded_column,
            global_filter,
            global_filterable_columns,
            filtering_mode,
            visit,
        );
    }
}

pub(super) fn row_matches_facet_filters(
    row: &TableResolvedRow,
    filters: &[TableFilter],
    excluded_column: &TableColumnId,
) -> bool {
    filters.iter().all(|filter| {
        filter.column() == excluded_column
            || row.source().is_some_and(|source| filter.matches(source))
    })
}

pub(super) fn resolved_row_matches_global_filter(
    row: &TableResolvedRow,
    global_filter: Option<&str>,
    global_filterable_columns: &[TableColumnId],
) -> bool {
    match row.source() {
        Some(source) => row_matches_global_filter(source, global_filter, global_filterable_columns),
        None => true,
    }
}

pub(super) fn row_matches_global_filter(
    row: &TableRow,
    global_filter: Option<&str>,
    global_filterable_columns: &[TableColumnId],
) -> bool {
    let Some(query) = global_filter else {
        return true;
    };
    if query.is_empty() {
        return true;
    }
    if global_filterable_columns.is_empty() {
        return false;
    }

    let query = query.to_lowercase();
    global_filterable_columns.iter().any(|column| {
        row.cell(column)
            .map(|value| value.filter_text().to_lowercase().contains(&query))
            .unwrap_or(false)
    })
}
