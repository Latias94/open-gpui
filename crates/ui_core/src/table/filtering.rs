//! Sorting and filtering contracts for renderer-neutral tables.

use std::collections::BTreeSet;

use super::{TableCellValue, TableColumnId, TableRow};

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

/// Column filter kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableFilterKind {
    /// Case-insensitive contains filter.
    Contains {
        /// Case-insensitive query text.
        query: String,
    },
    /// Exact categorical filter over stable facet tokens.
    OneOf {
        /// Exact stable facet tokens.
        values: BTreeSet<String>,
    },
    /// Inclusive numeric range filter with optional finite endpoints.
    NumberRange {
        /// Inclusive lower bound.
        min: Option<TableNumericFilterBound>,
        /// Inclusive upper bound.
        max: Option<TableNumericFilterBound>,
    },
    /// Text predicate filter.
    Text {
        /// Text operator.
        operator: TableTextFilterOperator,
        /// Query text.
        query: String,
        /// Whether matching should preserve case.
        case_sensitive: bool,
    },
    /// Single-bound numeric comparison filter.
    NumberComparison {
        /// Numeric comparison operator.
        operator: TableNumericFilterOperator,
        /// Finite comparison value.
        value: TableNumericFilterBound,
    },
}

/// Built-in text predicate operators for table column filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableTextFilterOperator {
    /// Cell text contains the query.
    Contains,
    /// Cell text does not contain the query.
    NotContains,
    /// Cell text equals the query.
    Equals,
    /// Cell text does not equal the query.
    NotEquals,
    /// Cell text starts with the query.
    StartsWith,
    /// Cell text ends with the query.
    EndsWith,
}

impl TableTextFilterOperator {
    /// Returns a stable operator label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::NotContains => "not_contains",
            Self::Equals => "equals",
            Self::NotEquals => "not_equals",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
        }
    }
}

/// Built-in numeric comparison operators for table column filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableNumericFilterOperator {
    /// Cell number is greater than the value.
    GreaterThan,
    /// Cell number is greater than or equal to the value.
    GreaterThanOrEqual,
    /// Cell number is less than the value.
    LessThan,
    /// Cell number is less than or equal to the value.
    LessThanOrEqual,
}

impl TableNumericFilterOperator {
    /// Returns a stable operator label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GreaterThan => "greater_than",
            Self::GreaterThanOrEqual => "greater_than_or_equal",
            Self::LessThan => "less_than",
            Self::LessThanOrEqual => "less_than_or_equal",
        }
    }
}

/// Finite numeric endpoint for table range filters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableNumericFilterBound(f64);

impl Eq for TableNumericFilterBound {}

impl TableNumericFilterBound {
    /// Creates a finite numeric filter bound.
    pub fn new(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }

        let value = if value == 0.0 { 0.0 } else { value };
        Some(Self(value))
    }

    /// Returns the numeric endpoint value.
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Column filter specification for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFilter {
    column: TableColumnId,
    kind: TableFilterKind,
}

impl TableFilter {
    /// Creates a case-insensitive contains filter.
    pub fn contains(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            kind: TableFilterKind::Contains {
                query: query.into(),
            },
        }
    }

    /// Creates an exact categorical filter over stable facet tokens.
    pub fn exact(column: impl Into<TableColumnId>, value: impl Into<String>) -> Self {
        Self::one_of(column, [value.into()])
    }

    /// Creates an exact categorical filter over multiple stable facet tokens.
    pub fn one_of(
        column: impl Into<TableColumnId>,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            column: column.into(),
            kind: TableFilterKind::OneOf {
                values: values.into_iter().map(Into::into).collect(),
            },
        }
    }

    /// Creates a text predicate filter.
    pub fn text(
        column: impl Into<TableColumnId>,
        operator: TableTextFilterOperator,
        query: impl Into<String>,
    ) -> Self {
        Self::text_with_case(column, operator, query, false)
    }

    /// Creates a text predicate filter with explicit case sensitivity.
    pub fn text_with_case(
        column: impl Into<TableColumnId>,
        operator: TableTextFilterOperator,
        query: impl Into<String>,
        case_sensitive: bool,
    ) -> Self {
        Self {
            column: column.into(),
            kind: TableFilterKind::Text {
                operator,
                query: query.into(),
                case_sensitive,
            },
        }
    }

    /// Creates a case-insensitive not-contains text filter.
    pub fn not_contains(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self::text(column, TableTextFilterOperator::NotContains, query)
    }

    /// Creates a case-insensitive exact text filter.
    pub fn text_equals(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self::text(column, TableTextFilterOperator::Equals, query)
    }

    /// Creates a case-insensitive not-equals text filter.
    pub fn text_not_equals(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self::text(column, TableTextFilterOperator::NotEquals, query)
    }

    /// Creates a case-insensitive starts-with text filter.
    pub fn starts_with(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self::text(column, TableTextFilterOperator::StartsWith, query)
    }

    /// Creates a case-insensitive ends-with text filter.
    pub fn ends_with(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self::text(column, TableTextFilterOperator::EndsWith, query)
    }

    /// Creates an inclusive numeric range filter.
    pub fn number_range(
        column: impl Into<TableColumnId>,
        min: Option<f64>,
        max: Option<f64>,
    ) -> Option<Self> {
        let min = min.and_then(TableNumericFilterBound::new);
        let max = max.and_then(TableNumericFilterBound::new);
        let (min, max) = normalize_table_numeric_filter_bounds(min, max);

        if min.is_none() && max.is_none() {
            return None;
        }

        Some(Self {
            column: column.into(),
            kind: TableFilterKind::NumberRange { min, max },
        })
    }

    /// Creates a single-bound numeric comparison filter.
    pub fn number_comparison(
        column: impl Into<TableColumnId>,
        operator: TableNumericFilterOperator,
        value: f64,
    ) -> Option<Self> {
        Some(Self {
            column: column.into(),
            kind: TableFilterKind::NumberComparison {
                operator,
                value: TableNumericFilterBound::new(value)?,
            },
        })
    }

    /// Creates a greater-than numeric comparison filter.
    pub fn number_greater_than(column: impl Into<TableColumnId>, value: f64) -> Option<Self> {
        Self::number_comparison(column, TableNumericFilterOperator::GreaterThan, value)
    }

    /// Creates a greater-than-or-equal numeric comparison filter.
    pub fn number_greater_than_or_equal(
        column: impl Into<TableColumnId>,
        value: f64,
    ) -> Option<Self> {
        Self::number_comparison(
            column,
            TableNumericFilterOperator::GreaterThanOrEqual,
            value,
        )
    }

    /// Creates a less-than numeric comparison filter.
    pub fn number_less_than(column: impl Into<TableColumnId>, value: f64) -> Option<Self> {
        Self::number_comparison(column, TableNumericFilterOperator::LessThan, value)
    }

    /// Creates a less-than-or-equal numeric comparison filter.
    pub fn number_less_than_or_equal(column: impl Into<TableColumnId>, value: f64) -> Option<Self> {
        Self::number_comparison(column, TableNumericFilterOperator::LessThanOrEqual, value)
    }

    /// Returns the filtered column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the filter kind.
    pub const fn kind(&self) -> &TableFilterKind {
        &self.kind
    }

    /// Returns the contains query when this is a contains filter.
    pub fn query(&self) -> &str {
        match &self.kind {
            TableFilterKind::Contains { query } => query,
            TableFilterKind::Text { query, .. } => query,
            TableFilterKind::OneOf { .. } => "",
            TableFilterKind::NumberRange { .. } => "",
            TableFilterKind::NumberComparison { .. } => "",
        }
    }

    /// Returns text predicate metadata when this is a text filter.
    pub fn text_predicate(&self) -> Option<(TableTextFilterOperator, &str, bool)> {
        match &self.kind {
            TableFilterKind::Contains { query } => {
                Some((TableTextFilterOperator::Contains, query.as_str(), false))
            }
            TableFilterKind::Text {
                operator,
                query,
                case_sensitive,
            } => Some((*operator, query.as_str(), *case_sensitive)),
            TableFilterKind::OneOf { .. }
            | TableFilterKind::NumberRange { .. }
            | TableFilterKind::NumberComparison { .. } => None,
        }
    }

    /// Returns the selected categorical tokens when this is an exact filter.
    pub fn selected_values(&self) -> Option<&BTreeSet<String>> {
        match &self.kind {
            TableFilterKind::Contains { .. } => None,
            TableFilterKind::Text { .. } => None,
            TableFilterKind::OneOf { values } => Some(values),
            TableFilterKind::NumberRange { .. } => None,
            TableFilterKind::NumberComparison { .. } => None,
        }
    }

    /// Returns numeric filter endpoints when this is a range filter.
    pub fn number_range_bounds(&self) -> Option<(Option<f64>, Option<f64>)> {
        match &self.kind {
            TableFilterKind::NumberRange { min, max } => Some((
                min.map(|bound| bound.value()),
                max.map(|bound| bound.value()),
            )),
            TableFilterKind::Contains { .. }
            | TableFilterKind::Text { .. }
            | TableFilterKind::OneOf { .. }
            | TableFilterKind::NumberComparison { .. } => None,
        }
    }

    /// Returns numeric comparison metadata when this is a single-bound comparison filter.
    pub fn number_comparison_value(&self) -> Option<(TableNumericFilterOperator, f64)> {
        match &self.kind {
            TableFilterKind::NumberComparison { operator, value } => {
                Some((*operator, value.value()))
            }
            TableFilterKind::Contains { .. }
            | TableFilterKind::Text { .. }
            | TableFilterKind::OneOf { .. }
            | TableFilterKind::NumberRange { .. } => None,
        }
    }

    pub(super) fn matches(&self, row: &TableRow) -> bool {
        match &self.kind {
            TableFilterKind::Contains { query } => {
                if query.is_empty() {
                    return true;
                }

                row.cell(&self.column)
                    .map(|value| {
                        value
                            .filter_text()
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                    })
                    .unwrap_or(false)
            }
            TableFilterKind::Text {
                operator,
                query,
                case_sensitive,
            } => {
                if query.is_empty() {
                    return true;
                }

                row.cell(&self.column)
                    .map(|value| {
                        table_text_filter_matches(
                            &value.filter_text(),
                            query,
                            *operator,
                            *case_sensitive,
                        )
                    })
                    .unwrap_or(false)
            }
            TableFilterKind::OneOf { values } => {
                if values.is_empty() {
                    return true;
                }

                row.cell(&self.column)
                    .map(|value| values.contains(&value.filter_text()))
                    .unwrap_or(false)
            }
            TableFilterKind::NumberRange { min, max } => {
                if min.is_none() && max.is_none() {
                    return true;
                }

                let Some(TableCellValue::Number(number)) = row.cell(&self.column) else {
                    return false;
                };
                if !number.is_finite() {
                    return false;
                }
                if min.is_some_and(|bound| *number < bound.value()) {
                    return false;
                }
                if max.is_some_and(|bound| *number > bound.value()) {
                    return false;
                }

                true
            }
            TableFilterKind::NumberComparison { operator, value } => {
                let Some(TableCellValue::Number(number)) = row.cell(&self.column) else {
                    return false;
                };
                if !number.is_finite() {
                    return false;
                }

                match operator {
                    TableNumericFilterOperator::GreaterThan => *number > value.value(),
                    TableNumericFilterOperator::GreaterThanOrEqual => *number >= value.value(),
                    TableNumericFilterOperator::LessThan => *number < value.value(),
                    TableNumericFilterOperator::LessThanOrEqual => *number <= value.value(),
                }
            }
        }
    }
}

fn table_text_filter_matches(
    value: &str,
    query: &str,
    operator: TableTextFilterOperator,
    case_sensitive: bool,
) -> bool {
    let (value, query) = if case_sensitive {
        (value.to_string(), query.to_string())
    } else {
        (value.to_lowercase(), query.to_lowercase())
    };

    match operator {
        TableTextFilterOperator::Contains => value.contains(&query),
        TableTextFilterOperator::NotContains => !value.contains(&query),
        TableTextFilterOperator::Equals => value == query,
        TableTextFilterOperator::NotEquals => value != query,
        TableTextFilterOperator::StartsWith => value.starts_with(&query),
        TableTextFilterOperator::EndsWith => value.ends_with(&query),
    }
}

fn normalize_table_numeric_filter_bounds(
    min: Option<TableNumericFilterBound>,
    max: Option<TableNumericFilterBound>,
) -> (
    Option<TableNumericFilterBound>,
    Option<TableNumericFilterBound>,
) {
    match (min, max) {
        (Some(left), Some(right)) if left.value() > right.value() => (Some(right), Some(left)),
        bounds => bounds,
    }
}

pub(super) fn normalize_table_global_filter_query(query: impl Into<String>) -> Option<String> {
    let query = query.into();
    let trimmed = query.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
