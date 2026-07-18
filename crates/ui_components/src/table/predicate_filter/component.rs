use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::geometry::gpui_px_from_ui;
use crate::listbox::ListboxOption;
use crate::select::Select;
use crate::text_input::TextInput;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use open_gpui_ui_core::{Sizable, Size, TableColumnId, TableTextFilterOperator, ThemeTokens};

use super::TablePredicateFilterChangeHandler;
use super::state::{
    TablePredicateFilterChange, TablePredicateFilterOperator, TablePredicateFilterState,
};

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
        let theme = ThemeResolver::current(window, cx);
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
            .text_color(theme.resolve(state.input().colors().foreground()))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                Select::new(select_id, select_label)
                    .with_size(size)
                    .selected(Some(state.operator().as_str().to_owned()))
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
                        .on_activate(move |_, window, cx| {
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
