use std::collections::BTreeSet;
use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::text_input::TextInput;
use crate::theme::ThemeContext;
use open_gpui::prelude::*;
use open_gpui::{
    App, Entity, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, px,
    rgba,
};
use open_gpui_ui_core::{Sizable, Size, TableColumnId, ThemeTokens, Toggled, UiPx};

use super::TableFacetedFilterChangeHandler;
use super::component::TableFacetedFilterRuntime;
use super::state::{
    TableFacetedFilterChange, TableFacetedFilterOptionState, TableFacetedFilterState,
};

pub(in crate::table::faceted_filter) fn table_faceted_filter_content_element(
    content_id: String,
    search_id: String,
    options_id: String,
    clear_id: String,
    state: TableFacetedFilterState,
    query_runtime: Entity<TableFacetedFilterRuntime>,
    on_query_change: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
    on_change: Option<TableFacetedFilterChangeHandler>,
    column_id: TableColumnId,
    selected_values: BTreeSet<String>,
    options_height: UiPx,
    size: Size,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let query = state.query().to_owned();
    let options = state.options().to_vec();
    let clear_enabled = state.clear_enabled();
    let clear_label = state.clear_label().to_owned();
    let placeholder = state
        .search_input()
        .placeholder()
        .unwrap_or("Search values")
        .to_owned();
    let selected_summary = if state.selected_labels().is_empty() {
        None
    } else {
        Some(state.selected_labels().join(", "))
    };
    let empty_label = state.empty_label().to_owned();
    let popup_query = query.clone();
    let query_runtime_for_input = query_runtime.clone();
    let query_on_change = on_query_change.clone();
    let state_for_query = state.clone();
    let column_id_for_toggle = column_id.clone();
    let on_change_for_clear = on_change.clone();
    let content_debug_id = state.id().to_owned();

    div()
        .id(content_id)
        .debug_selector(move || format!("table-faceted-filter:{content_debug_id}:content"))
        .min_w(px(0.0))
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .text_color(theme.resolve(state.popover().colors().foreground()))
        .on_scroll_wheel(|_, _, _| open_gpui::ScrollWheelIntent::handled().stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .truncate()
                        .child(state.trigger_label().to_owned()),
                )
                .when_some(selected_summary, |this, summary| {
                    this.child(div().flex_none().text_xs().opacity(0.72).child(summary))
                }),
        )
        .child(
            TextInput::new(search_id, state.label().to_owned())
                .with_size(size)
                .value(popup_query)
                .placeholder(placeholder)
                .disabled(disabled)
                .tokens(tokens)
                .on_change(move |next_query, window, cx| {
                    query_runtime_for_input.update(cx, |runtime, _| {
                        runtime.query = next_query.clone();
                    });
                    if let Some(on_query_change) = query_on_change.as_ref() {
                        on_query_change(next_query, window, cx);
                    }
                }),
        )
        .when(clear_enabled, |this| {
            this.child(
                div().flex().justify_end().child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                let change =
                                    TableFacetedFilterChange::clear(column_id_for_toggle.clone());
                                on_change(change, window, cx);
                            }
                        }),
                ),
            )
        })
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .h(gpui_px_from_ui(options_height))
                .overflow_hidden()
                .child(
                    ScrollArea::new(
                        options_id,
                        table_faceted_filter_options_element(
                            state_for_query,
                            options,
                            query_runtime,
                            on_change,
                            column_id,
                            selected_values,
                            empty_label,
                            disabled,
                        ),
                    )
                    .vertical()
                    .reset_on_key(query.clone())
                    .with_size(size),
                ),
        )
}

fn table_faceted_filter_options_element(
    state: TableFacetedFilterState,
    options: Vec<TableFacetedFilterOptionState>,
    query_runtime: Entity<TableFacetedFilterRuntime>,
    on_change: Option<TableFacetedFilterChangeHandler>,
    column_id: TableColumnId,
    selected_values: BTreeSet<String>,
    empty_label: String,
    disabled: bool,
) -> impl IntoElement {
    if options.is_empty() {
        return div()
            .min_w(px(0.0))
            .py(px(4.0))
            .text_sm()
            .opacity(0.72)
            .child(empty_label)
            .into_any_element();
    }

    let query = state.query().to_owned();

    options
        .into_iter()
        .fold(
            div().flex().flex_col().gap_1().min_w(px(0.0)),
            |list, option| {
                let option_value = option.value().to_owned();
                let option_label = option.label().to_owned();
                let option_count = option.count();
                let option_checked = option.selected();
                let option_selected_values = selected_values.clone();
                let on_change = on_change.clone();
                let column_id = column_id.clone();
                let query_runtime_for_toggle = query_runtime.clone();
                let query_for_toggle = query.clone();
                let option_id = format!("{}-option-{option_value}", state.id());
                let row_id = format!("{}-option-row-{option_value}", state.id());
                let option_debug_id = state.id().to_owned();
                let option_debug_value = option_value.clone();
                let row_selected_values = selected_values.clone();
                let row_on_change = on_change.clone();
                let row_column_id = column_id.clone();
                let row_query_runtime = query_runtime.clone();
                let row_query = query.clone();
                let row_option_value = option_value.clone();

                list.child(
                    div()
                        .id(row_id)
                        .debug_selector(move || {
                            format!(
                                "table-faceted-filter:{option_debug_id}:option:{option_debug_value}"
                            )
                        })
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .rounded(px(6.0))
                        .px(px(6.0))
                        .py(px(4.0))
                        .when(disabled, |this| this.opacity(0.56))
                        .when(!disabled, move |this| {
                            this.cursor_pointer()
                                .hover(|style| style.bg(rgba(0x00000010)))
                                .on_click(move |_, window, cx| {
                                    let mut next_values = row_selected_values.clone();
                                    let next_selected = !option_checked;
                                    if next_selected {
                                        next_values.insert(row_option_value.clone());
                                    } else {
                                        next_values.remove(&row_option_value);
                                    }
                                    row_query_runtime.update(cx, |runtime, _| {
                                        runtime.query = row_query.clone();
                                    });
                                    if let Some(on_change) = row_on_change.as_ref() {
                                        let change = TableFacetedFilterChange::new(
                                            row_column_id.clone(),
                                            next_values.into_iter(),
                                            Some(row_option_value.clone()),
                                            next_selected,
                                        );
                                        on_change(change, window, cx);
                                    }
                                })
                        })
                        .child(
                            Checkbox::new(option_id)
                                .label(option_label.clone())
                                .checked(option_checked)
                                .disabled(disabled)
                                .on_toggle(move |toggled, _event, window, cx| {
                                    let mut next_values = option_selected_values.clone();
                                    match toggled {
                                        Toggled::True => {
                                            next_values.insert(option_value.clone());
                                        }
                                        Toggled::False | Toggled::Mixed => {
                                            next_values.remove(&option_value);
                                        }
                                    }
                                    query_runtime_for_toggle.update(cx, |runtime, _| {
                                        runtime.query = query_for_toggle.clone();
                                    });
                                    if let Some(on_change) = on_change.as_ref() {
                                        let change = TableFacetedFilterChange::new(
                                            column_id.clone(),
                                            next_values.into_iter(),
                                            Some(option_value.clone()),
                                            matches!(toggled, Toggled::True),
                                        );
                                        on_change(change, window, cx);
                                    }
                                }),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .opacity(0.72)
                                .child(option_count.to_string()),
                        ),
                )
            },
        )
        .into_any_element()
}
