use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::geometry::gpui_px_from_ui;
use crate::listbox::ListboxOption;
use crate::select::{Select, SelectState};
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use open_gpui_ui_core::{
    Sizable, Size, TableColumnId, TableFilter, TableNumericFilterOperator, TableState,
    TableTextFilterOperator, ThemeTokens,
};

use super::filtering::{
    table_predicate_filter_next_filters, table_predicate_filter_operator_options,
};

type TablePredicateFilterChangeHandler =
    Rc<dyn Fn(TablePredicateFilterChange, &mut Window, &mut App)>;
/// Supported predicate operator families for the table predicate filter recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePredicateFilterOperator {
    /// A text predicate backed by [`TableTextFilterOperator`].
    Text(TableTextFilterOperator),
    /// A numeric predicate backed by [`TableNumericFilterOperator`].
    Number(TableNumericFilterOperator),
}

impl TablePredicateFilterOperator {
    /// Creates a text operator wrapper.
    pub const fn text(operator: TableTextFilterOperator) -> Self {
        Self::Text(operator)
    }

    /// Creates a numeric operator wrapper.
    pub const fn number(operator: TableNumericFilterOperator) -> Self {
        Self::Number(operator)
    }

    /// Returns a stable operator value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text(TableTextFilterOperator::Contains) => "text:contains",
            Self::Text(TableTextFilterOperator::NotContains) => "text:not_contains",
            Self::Text(TableTextFilterOperator::Equals) => "text:equals",
            Self::Text(TableTextFilterOperator::NotEquals) => "text:not_equals",
            Self::Text(TableTextFilterOperator::StartsWith) => "text:starts_with",
            Self::Text(TableTextFilterOperator::EndsWith) => "text:ends_with",
            Self::Number(TableNumericFilterOperator::GreaterThan) => "number:greater_than",
            Self::Number(TableNumericFilterOperator::GreaterThanOrEqual) => {
                "number:greater_than_or_equal"
            }
            Self::Number(TableNumericFilterOperator::LessThan) => "number:less_than",
            Self::Number(TableNumericFilterOperator::LessThanOrEqual) => {
                "number:less_than_or_equal"
            }
        }
    }

    /// Returns the visible operator label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Text(TableTextFilterOperator::Contains) => "Contains",
            Self::Text(TableTextFilterOperator::NotContains) => "Does not contain",
            Self::Text(TableTextFilterOperator::Equals) => "Equals",
            Self::Text(TableTextFilterOperator::NotEquals) => "Does not equal",
            Self::Text(TableTextFilterOperator::StartsWith) => "Starts with",
            Self::Text(TableTextFilterOperator::EndsWith) => "Ends with",
            Self::Number(TableNumericFilterOperator::GreaterThan) => "Greater than",
            Self::Number(TableNumericFilterOperator::GreaterThanOrEqual) => "Greater than or equal",
            Self::Number(TableNumericFilterOperator::LessThan) => "Less than",
            Self::Number(TableNumericFilterOperator::LessThanOrEqual) => "Less than or equal",
        }
    }

    /// Returns the wrapped text operator, when available.
    pub const fn text_operator(self) -> Option<TableTextFilterOperator> {
        match self {
            Self::Text(operator) => Some(operator),
            Self::Number(_) => None,
        }
    }

    /// Returns the wrapped numeric operator, when available.
    pub const fn numeric_operator(self) -> Option<TableNumericFilterOperator> {
        match self {
            Self::Text(_) => None,
            Self::Number(operator) => Some(operator),
        }
    }

    /// Resolves a stable operator wrapper from the serialized value.
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "text:contains" => Some(Self::text(TableTextFilterOperator::Contains)),
            "text:not_contains" => Some(Self::text(TableTextFilterOperator::NotContains)),
            "text:equals" => Some(Self::text(TableTextFilterOperator::Equals)),
            "text:not_equals" => Some(Self::text(TableTextFilterOperator::NotEquals)),
            "text:starts_with" => Some(Self::text(TableTextFilterOperator::StartsWith)),
            "text:ends_with" => Some(Self::text(TableTextFilterOperator::EndsWith)),
            "number:greater_than" => Some(Self::number(TableNumericFilterOperator::GreaterThan)),
            "number:greater_than_or_equal" => {
                Some(Self::number(TableNumericFilterOperator::GreaterThanOrEqual))
            }
            "number:less_than" => Some(Self::number(TableNumericFilterOperator::LessThan)),
            "number:less_than_or_equal" => {
                Some(Self::number(TableNumericFilterOperator::LessThanOrEqual))
            }
            _ => None,
        }
    }

    /// Builds the matching table filter for a supplied value.
    pub fn filter(self, column_id: impl Into<TableColumnId>, value: &str) -> Option<TableFilter> {
        let column_id = column_id.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        match self {
            Self::Text(operator) => Some(TableFilter::text(column_id, operator, trimmed)),
            Self::Number(operator) => trimmed
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .and_then(|value| TableFilter::number_comparison(column_id, operator, value)),
        }
    }
}

/// One selectable operator row in a table predicate filter recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePredicateFilterOperatorOptionState {
    operator: TablePredicateFilterOperator,
    label: String,
    selected: bool,
}

impl TablePredicateFilterOperatorOptionState {
    pub(super) fn new(
        operator: TablePredicateFilterOperator,
        label: impl Into<String>,
        selected: bool,
    ) -> Self {
        Self {
            operator,
            label: label.into(),
            selected,
        }
    }

    /// Returns the resolved operator.
    pub const fn operator(&self) -> TablePredicateFilterOperator {
        self.operator
    }

    /// Returns the stable serialized operator value.
    pub fn value(&self) -> &'static str {
        self.operator.as_str()
    }

    /// Returns the visible label for this operator option.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this option is currently selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Controlled payload emitted when a table predicate filter changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePredicateFilterChange {
    column_id: TableColumnId,
    operator: Option<TablePredicateFilterOperator>,
    value: String,
    cleared: bool,
}

impl TablePredicateFilterChange {
    /// Creates a predicate-change payload from the current operator and value.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        operator: TablePredicateFilterOperator,
        value: impl Into<String>,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            operator: Some(operator),
            value: value.into(),
            cleared: false,
        }
    }

    /// Creates a payload that clears this column's predicate filter.
    pub fn clear(column_id: impl Into<TableColumnId>) -> Self {
        Self {
            column_id: column_id.into(),
            operator: None,
            value: String::new(),
            cleared: true,
        }
    }

    /// Returns the filtered column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the selected operator, when present.
    pub const fn operator(&self) -> Option<TablePredicateFilterOperator> {
        self.operator
    }

    /// Returns the raw value exactly as entered.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns true when this payload was created by a clear action.
    pub const fn cleared(&self) -> bool {
        self.cleared
    }

    /// Returns whether this payload resolves to an active table filter.
    pub fn active(&self) -> bool {
        self.filter().is_some()
    }

    /// Returns the next filter, when the payload resolves to one.
    pub fn filter(&self) -> Option<TableFilter> {
        if self.cleared {
            return None;
        }

        self.operator
            .and_then(|operator| operator.filter(self.column_id.clone(), &self.value))
    }

    /// Returns the next column-filter list while preserving unrelated filters.
    pub fn next_filters(&self, filters: impl IntoIterator<Item = TableFilter>) -> Vec<TableFilter> {
        table_predicate_filter_next_filters(filters, &self.column_id, self.filter())
    }

    /// Applies this predicate change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_filters = self.next_filters(state.filters().iter().cloned());
        let next_pagination = state.pagination().with_page_index(0);

        state
            .with_filters(next_filters)
            .with_pagination(next_pagination)
    }
}

/// Resolved renderer-neutral state for a table predicate filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TablePredicateFilterState {
    id: String,
    label: String,
    column_id: TableColumnId,
    operator: TablePredicateFilterOperator,
    value: String,
    placeholder: String,
    clear_label: String,
    operator_options: Vec<TablePredicateFilterOperatorOptionState>,
    select: SelectState,
    input: TextInputState,
}

impl TablePredicateFilterState {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        column_id: TableColumnId,
        operator: TablePredicateFilterOperator,
        value: impl Into<String>,
        operator_options: impl IntoIterator<Item = (TablePredicateFilterOperator, SharedString)>,
        placeholder: impl Into<String>,
        clear_label: impl Into<String>,
        size: Size,
        disabled: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let id = id.into();
        let label = label.into();
        let value = value.into();
        let placeholder = placeholder.into();
        let clear_label = clear_label.into();
        let operator_options = table_predicate_filter_operator_options(operator, operator_options);
        let select = Select::new(format!("{id}-operator"), format!("{label} operator"))
            .options(
                operator_options
                    .iter()
                    .map(|option| ListboxOption::new(option.value(), option.label().to_owned()))
                    .collect::<Vec<_>>(),
            )
            .selected(operator.as_str())
            .placeholder("Operator")
            .with_size(size)
            .disabled(disabled)
            .tokens(tokens)
            .state();
        let input = TextInputState::resolve(
            value.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );

        Self {
            id,
            label,
            column_id,
            operator,
            value,
            placeholder,
            clear_label,
            operator_options,
            select,
            input,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible filter label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the filtered column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the current operator.
    pub const fn operator(&self) -> TablePredicateFilterOperator {
        self.operator
    }

    /// Returns the raw value exactly as entered.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the input placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns the clear button label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns whether the predicate currently resolves to an active filter.
    pub fn active(&self) -> bool {
        self.operator
            .filter(self.column_id.clone(), &self.value)
            .is_some()
    }

    /// Returns whether the clear action should be available.
    pub fn clear_enabled(&self) -> bool {
        !self.value.trim().is_empty()
    }

    /// Returns the available operator options in stable order.
    pub fn operator_options(&self) -> &[TablePredicateFilterOperatorOptionState] {
        &self.operator_options
    }

    /// Returns resolved select state for the operator control.
    pub const fn select(&self) -> &SelectState {
        &self.select
    }

    /// Returns resolved text input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns the foundation size from the nested controls.
    pub const fn size(&self) -> Size {
        self.input.size()
    }

    /// Returns whether the predicate controls are disabled.
    pub const fn disabled(&self) -> bool {
        self.input.disabled()
    }
}

#[derive(Debug, Clone)]
struct TablePredicateFilterRuntime {
    operator: TablePredicateFilterOperator,
    value: String,
}

/// A compact operator select + text input recipe for one table column predicate.
#[derive(IntoElement)]
pub struct TablePredicateFilter {
    id: String,
    label: SharedString,
    column_id: TableColumnId,
    operator: Option<TablePredicateFilterOperator>,
    default_operator: TablePredicateFilterOperator,
    value: Option<String>,
    default_value: String,
    operator_options: Vec<(TablePredicateFilterOperator, SharedString)>,
    placeholder: SharedString,
    clear_label: SharedString,
    size: Size,
    disabled: bool,
    tokens: ThemeTokens,
    on_change: Option<TablePredicateFilterChangeHandler>,
}

impl TablePredicateFilter {
    /// Creates a predicate filter recipe for one table column.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        column_id: impl Into<TableColumnId>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            column_id: column_id.into(),
            operator: None,
            default_operator: TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
            value: None,
            default_value: String::new(),
            operator_options: Vec::new(),
            placeholder: "Filter value".into(),
            clear_label: "Clear filter".into(),
            size: Size::Medium,
            disabled: false,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Applies controlled operator state.
    pub fn operator(mut self, operator: TablePredicateFilterOperator) -> Self {
        self.operator = Some(operator);
        self
    }

    /// Applies the default operator for adapter-owned state.
    pub fn default_operator(mut self, operator: TablePredicateFilterOperator) -> Self {
        self.default_operator = operator;
        self
    }

    /// Applies controlled value text.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Applies the default value for adapter-owned state.
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = value.into();
        self
    }

    /// Adds one operator option with an explicit label.
    pub fn operator_option(
        mut self,
        operator: TablePredicateFilterOperator,
        label: impl Into<SharedString>,
    ) -> Self {
        self.operator_options.push((operator, label.into()));
        self
    }

    /// Adds many operator options using the stable operator defaults.
    pub fn operators(
        mut self,
        operators: impl IntoIterator<Item = TablePredicateFilterOperator>,
    ) -> Self {
        self.operator_options
            .extend(operators.into_iter().map(|operator| {
                let label = operator.label();
                (operator, SharedString::from(label))
            }));
        self
    }

    /// Applies input placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies the clear button label.
    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Marks the filter controls as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a predicate-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TablePredicateFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TablePredicateFilterState {
        let operator = self.operator.unwrap_or(self.default_operator);
        let value = self.value.as_deref().unwrap_or(self.default_value.as_str());
        TablePredicateFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.column_id.clone(),
            operator,
            value,
            self.operator_options.clone(),
            self.placeholder.to_string(),
            self.clear_label.to_string(),
            self.size,
            self.disabled,
            self.tokens,
        )
    }
}

impl Sizable for TablePredicateFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TablePredicateFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TablePredicateFilterRuntime {
            operator: self.default_operator,
            value: self.default_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_operator = self.operator;
        let controlled_value = self.value.clone();
        let operator = controlled_operator.unwrap_or(runtime_state.operator);
        let value = controlled_value.clone().unwrap_or(runtime_state.value);

        if controlled_operator.is_some() && runtime.read(cx).operator != operator {
            runtime.update(cx, |runtime, _| {
                runtime.operator = operator;
            });
        }
        if controlled_value.is_some() && runtime.read(cx).value != value {
            runtime.update(cx, |runtime, _| {
                runtime.value = value.clone();
            });
        }

        let state = TablePredicateFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            self.column_id.clone(),
            operator,
            value.clone(),
            self.operator_options.clone(),
            self.placeholder.clone(),
            self.clear_label.clone(),
            self.size,
            self.disabled,
            self.tokens,
        );
        let debug_id = state.id().to_owned();
        let label = state.label().to_owned();
        let placeholder = state.placeholder().to_owned();
        let clear_label = state.clear_label().to_owned();
        let disabled = state.disabled();
        let size = state.size();
        let select_id = format!("{}-operator", self.id);
        let input_id = format!("{}-value", self.id);
        let clear_id = format!("{}-clear", self.id);
        let column_id_for_select = self.column_id.clone();
        let column_id_for_input = self.column_id.clone();
        let column_id_for_clear = self.column_id.clone();
        let runtime_for_select = runtime.clone();
        let runtime_for_input = runtime.clone();
        let runtime_for_clear = runtime.clone();
        let on_change_for_select = self.on_change.clone();
        let on_change_for_input = self.on_change.clone();
        let on_change_for_clear = self.on_change.clone();
        let select_label = format!("{label} operator");

        div()
            .id(self.id)
            .debug_selector(move || format!("table-predicate-filter:{debug_id}:root"))
            .min_w(px(0.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(ThemeResolver::resolve(state.input().colors().foreground()))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                Select::new(select_id, select_label)
                    .with_size(size)
                    .selected(state.operator().as_str())
                    .options(
                        state
                            .operator_options()
                            .iter()
                            .map(|option| {
                                ListboxOption::new(option.value(), option.label().to_owned())
                            })
                            .collect::<Vec<_>>(),
                    )
                    .disabled(disabled)
                    .tokens(self.tokens)
                    .on_select(move |selection, window, cx| {
                        let Some(next_operator) =
                            TablePredicateFilterOperator::from_str(selection.value())
                        else {
                            return;
                        };
                        runtime_for_select.update(cx, |runtime, _| {
                            runtime.operator = next_operator;
                        });
                        if let Some(on_change) = on_change_for_select.as_ref() {
                            on_change(
                                TablePredicateFilterChange::new(
                                    column_id_for_select.clone(),
                                    next_operator,
                                    runtime_for_select.read(cx).value.clone(),
                                ),
                                window,
                                cx,
                            );
                        }
                    }),
            )
            .child(
                div().min_w(px(0.0)).flex_1().child(
                    TextInput::new(input_id, label)
                        .with_size(size)
                        .value(value)
                        .placeholder(placeholder)
                        .disabled(disabled)
                        .tokens(self.tokens)
                        .on_change(move |next_value, window, cx| {
                            runtime_for_input.update(cx, |runtime, _| {
                                runtime.value = next_value.clone();
                            });
                            if let Some(on_change) = on_change_for_input.as_ref() {
                                on_change(
                                    TablePredicateFilterChange::new(
                                        column_id_for_input.clone(),
                                        runtime_for_input.read(cx).operator,
                                        next_value,
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
            )
            .when(state.clear_enabled(), |this| {
                this.child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.value.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(
                                    TablePredicateFilterChange::clear(column_id_for_clear.clone()),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
            })
    }
}
