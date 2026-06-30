//! Aggregation contracts and grouped-row aggregate resolution.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use super::{TableCellValue, TableColumnId, TableResolvedRow};

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

    /// Resolves a stable label back to a built-in aggregate kind.
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "count" => Some(Self::Count),
            "sum" => Some(Self::Sum),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "average" => Some(Self::Average),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableAggregationSpec {
    BuiltIn(TableAggregateKind),
    Named(String),
}

#[derive(Clone)]
pub(super) struct TableAggregationFn(
    Arc<dyn Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue + Send + Sync>,
);

impl TableAggregationFn {
    pub(super) fn new(
        aggregation_fn: impl Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self(Arc::new(aggregation_fn))
    }

    pub(super) fn call(&self, column: &TableColumnId, rows: &[TableResolvedRow]) -> TableCellValue {
        (self.0)(column, rows)
    }
}

impl fmt::Debug for TableAggregationFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TableAggregationFn(..)")
    }
}

impl PartialEq for TableAggregationFn {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for TableAggregationFn {}

/// Aggregate specification for one table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableAggregation {
    column: TableColumnId,
    spec: TableAggregationSpec,
}

impl TableAggregation {
    /// Creates an aggregate specification for a column.
    pub fn new(column: impl Into<TableColumnId>, kind: TableAggregateKind) -> Self {
        Self {
            column: column.into(),
            spec: TableAggregationSpec::BuiltIn(kind),
        }
    }

    /// Creates a named aggregate specification for a column.
    pub fn named(column: impl Into<TableColumnId>, name: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            spec: TableAggregationSpec::Named(name.into()),
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

    /// Returns the aggregate kind when this is a built-in aggregate.
    pub fn kind(&self) -> Option<TableAggregateKind> {
        match self.spec {
            TableAggregationSpec::BuiltIn(kind) => Some(kind),
            TableAggregationSpec::Named(_) => None,
        }
    }

    /// Returns the named aggregate callback key, when present.
    pub fn name(&self) -> Option<&str> {
        match &self.spec {
            TableAggregationSpec::BuiltIn(_) => None,
            TableAggregationSpec::Named(name) => Some(name.as_str()),
        }
    }
}

pub(super) fn resolve_aggregate_cells(
    rows: &[TableResolvedRow],
    aggregations: &[TableAggregation],
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
) -> BTreeMap<TableColumnId, TableCellValue> {
    aggregations
        .iter()
        .map(|aggregation| {
            (
                aggregation.column().clone(),
                resolve_aggregate_cell(rows, aggregation, aggregation_fns),
            )
        })
        .collect()
}

fn resolve_aggregate_cell(
    rows: &[TableResolvedRow],
    aggregation: &TableAggregation,
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
) -> TableCellValue {
    match aggregation.kind() {
        Some(kind) => resolve_aggregate_cell_builtin(rows, aggregation.column(), kind),
        None => match aggregation.name() {
            Some(name) => aggregation_fns
                .get(name)
                .map(|aggregation_fn| aggregation_fn.call(aggregation.column(), rows))
                .or_else(|| {
                    TableAggregateKind::from_str(name).map(|kind| {
                        resolve_aggregate_cell_builtin(rows, aggregation.column(), kind)
                    })
                })
                .unwrap_or_default(),
            None => TableCellValue::Empty,
        },
    }
}

fn resolve_aggregate_cell_builtin(
    rows: &[TableResolvedRow],
    column: &TableColumnId,
    kind: TableAggregateKind,
) -> TableCellValue {
    match kind {
        TableAggregateKind::Count => TableCellValue::Number(rows.len() as f64),
        TableAggregateKind::Sum => {
            let mut seen_numeric = false;
            let sum = numeric_values(rows, column).fold(0.0, |sum, value| {
                seen_numeric = true;
                sum + value
            });

            if seen_numeric {
                TableCellValue::Number(sum)
            } else {
                TableCellValue::Empty
            }
        }
        TableAggregateKind::Min => numeric_values(rows, column)
            .min_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Max => numeric_values(rows, column)
            .max_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Average => {
            let mut count = 0_usize;
            let sum = numeric_values(rows, column).fold(0.0, |sum, value| {
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

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn numeric_values<'a>(
    rows: &'a [TableResolvedRow],
    column: &'a TableColumnId,
) -> impl Iterator<Item = f64> + 'a {
    rows.iter().filter_map(|row| match row.cell(column) {
        Some(TableCellValue::Number(value)) => Some(*value),
        _ => None,
    })
}
