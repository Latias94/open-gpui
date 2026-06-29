use std::collections::BTreeSet;

use open_gpui::SharedString;
use open_gpui_ui_core::{
    TableCellValue, TableColumnId, TableFilter, TableNumericFilterOperator, TableTextFilterOperator,
};

use super::{
    TableFacetedFilterOptionState, TablePredicateFilterOperator,
    TablePredicateFilterOperatorOptionState,
};

pub(super) fn normalize_table_faceted_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub(super) fn table_facet_value_label(value: &TableCellValue) -> String {
    let label = value.filter_text();
    if label.is_empty() {
        String::from("(empty)")
    } else {
        label
    }
}

pub(super) fn table_faceted_option_matches(
    option: &TableFacetedFilterOptionState,
    query: &str,
) -> bool {
    if query.is_empty() {
        return true;
    }

    option.label().to_lowercase().contains(query) || option.value().to_lowercase().contains(query)
}

pub(super) fn table_faceted_selected_labels(
    options: &[TableFacetedFilterOptionState],
    selected_values: &BTreeSet<String>,
) -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = BTreeSet::new();

    for option in options {
        if selected_values.contains(option.value()) && seen.insert(option.value().to_owned()) {
            labels.push(option.label().to_owned());
        }
    }

    for value in selected_values {
        if seen.insert(value.clone()) {
            labels.push(table_faceted_selected_label_for_value(value));
        }
    }

    labels
}

fn table_faceted_selected_label_for_value(value: &str) -> String {
    if value.is_empty() {
        String::from("(empty)")
    } else {
        value.to_owned()
    }
}

pub(super) fn table_faceted_trigger_label(label: &str, selected_labels: &[String]) -> String {
    match selected_labels.len() {
        0 => label.to_owned(),
        1 => format!("{label}: {}", selected_labels[0]),
        2 => format!("{label}: {}, {}", selected_labels[0], selected_labels[1]),
        count => format!("{label}: {count} selected"),
    }
}

pub(super) fn table_faceted_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    selected_values: &[String],
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| filter.column() != column_id)
        .collect::<Vec<_>>();

    if !selected_values.is_empty() {
        next.push(TableFilter::one_of(
            column_id.clone(),
            selected_values.iter().cloned(),
        ));
    }

    next
}

pub(super) fn table_range_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    min: Option<f64>,
    max: Option<f64>,
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| filter.column() != column_id || filter.number_range_bounds().is_none())
        .collect::<Vec<_>>();

    if let Some(filter) = TableFilter::number_range(column_id.clone(), min, max) {
        next.push(filter);
    }

    next
}

pub(super) fn parse_table_range_filter_bound(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| if value == 0.0 { 0.0 } else { value })
}

pub(super) fn normalize_table_range_filter_values(
    min: Option<f64>,
    max: Option<f64>,
) -> (Option<f64>, Option<f64>) {
    match (min, max) {
        (Some(left), Some(right)) if left > right => (Some(right), Some(left)),
        values => values,
    }
}

pub(super) fn table_range_filter_value_text(value: Option<f64>) -> String {
    value
        .map(|value| {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        })
        .unwrap_or_default()
}

pub(super) fn table_range_filter_trigger_label(
    label: &str,
    min: Option<f64>,
    max: Option<f64>,
) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!(
            "{label}: {}-{}",
            table_range_filter_value_text(Some(min)),
            table_range_filter_value_text(Some(max))
        ),
        (Some(min), None) => {
            format!("{label}: >= {}", table_range_filter_value_text(Some(min)))
        }
        (None, Some(max)) => {
            format!("{label}: <= {}", table_range_filter_value_text(Some(max)))
        }
        (None, None) => label.to_owned(),
    }
}

pub(super) fn table_range_filter_bound_placeholder(prefix: &str, value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{prefix} ({})", table_range_filter_value_text(Some(value))),
        None => prefix.to_owned(),
    }
}

pub(super) fn table_predicate_filter_operator_options(
    selected_operator: TablePredicateFilterOperator,
    configured: impl IntoIterator<Item = (TablePredicateFilterOperator, SharedString)>,
) -> Vec<TablePredicateFilterOperatorOptionState> {
    let mut options = configured
        .into_iter()
        .map(|(operator, label)| {
            TablePredicateFilterOperatorOptionState::new(
                operator,
                label.to_string(),
                operator == selected_operator,
            )
        })
        .collect::<Vec<_>>();

    if options.is_empty() {
        options = default_table_predicate_filter_operator_options(selected_operator);
    } else if !options
        .iter()
        .any(|option| option.operator() == selected_operator)
    {
        options.insert(
            0,
            TablePredicateFilterOperatorOptionState::new(
                selected_operator,
                selected_operator.label(),
                true,
            ),
        );
    }

    options
}

fn default_table_predicate_filter_operator_options(
    selected_operator: TablePredicateFilterOperator,
) -> Vec<TablePredicateFilterOperatorOptionState> {
    [
        TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
        TablePredicateFilterOperator::text(TableTextFilterOperator::NotContains),
        TablePredicateFilterOperator::text(TableTextFilterOperator::Equals),
        TablePredicateFilterOperator::text(TableTextFilterOperator::NotEquals),
        TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith),
        TablePredicateFilterOperator::text(TableTextFilterOperator::EndsWith),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThan),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThanOrEqual),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThan),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThanOrEqual),
    ]
    .into_iter()
    .map(|operator| {
        TablePredicateFilterOperatorOptionState::new(
            operator,
            operator.label(),
            operator == selected_operator,
        )
    })
    .collect()
}

pub(super) fn table_predicate_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    filter: Option<TableFilter>,
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| {
            if filter.column() != column_id {
                return true;
            }

            filter.text_predicate().is_none() && filter.number_comparison_value().is_none()
        })
        .collect::<Vec<_>>();

    if let Some(filter) = filter {
        next.push(filter);
    }

    next
}
