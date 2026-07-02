use crate::button::{Button, ButtonVariant};
use crate::text_input::TextInput;
use crate::theme::ThemeContext;
use open_gpui::prelude::*;
use open_gpui::{Entity, IntoElement, ParentElement, Styled, div, px};
use open_gpui_ui_core::{Sizable, Size, TableColumnId, ThemeTokens};

use super::TableRangeFilterChangeHandler;
use super::component::TableRangeFilterRuntime;
use super::state::{TableRangeFilterChange, TableRangeFilterState};
use crate::table::filtering::table_range_filter_value_text;

pub(in crate::table::range_filter) fn table_range_filter_content_element(
    content_id: String,
    min_id: String,
    max_id: String,
    clear_id: String,
    state: TableRangeFilterState,
    runtime: Entity<TableRangeFilterRuntime>,
    on_change: Option<TableRangeFilterChangeHandler>,
    column_id: TableColumnId,
    size: Size,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let min_text = state.min_text().to_owned();
    let max_text = state.max_text().to_owned();
    let clear_enabled = state.clear_enabled();
    let clear_label = state.clear_label().to_owned();
    let min_placeholder = state.min_placeholder().to_owned();
    let max_placeholder = state.max_placeholder().to_owned();
    let facet_range_text = state.facet_range().map(|range| {
        format!(
            "{} - {}",
            table_range_filter_value_text(Some(range.min())),
            table_range_filter_value_text(Some(range.max()))
        )
    });
    let runtime_for_min = runtime.clone();
    let runtime_for_max = runtime.clone();
    let on_change_for_min = on_change.clone();
    let on_change_for_max = on_change.clone();
    let column_id_for_min = column_id.clone();
    let column_id_for_max = column_id.clone();
    let content_debug_id = state.id().to_owned();

    div()
        .id(content_id)
        .debug_selector(move || format!("table-range-filter:{content_debug_id}:content"))
        .min_w(px(0.0))
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .text_color(theme.resolve(state.popover().colors().foreground()))
        .on_scroll_wheel(|_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
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
                .when_some(facet_range_text, |this, text| {
                    this.child(div().flex_none().text_xs().opacity(0.72).child(text))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    TextInput::new(min_id, format!("{} minimum", state.label()))
                        .value(min_text.clone())
                        .placeholder(min_placeholder)
                        .disabled(disabled)
                        .with_size(size)
                        .tokens(tokens)
                        .on_change(move |next_min, window, cx| {
                            runtime_for_min.update(cx, |runtime, _| {
                                runtime.min_text = next_min.clone();
                            });
                            if let Some(on_change) = on_change_for_min.as_ref() {
                                on_change(
                                    TableRangeFilterChange::new(
                                        column_id_for_min.clone(),
                                        next_min,
                                        runtime_for_min.read(cx).max_text.clone(),
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    TextInput::new(max_id, format!("{} maximum", state.label()))
                        .value(max_text.clone())
                        .placeholder(max_placeholder)
                        .disabled(disabled)
                        .with_size(size)
                        .tokens(tokens)
                        .on_change(move |next_max, window, cx| {
                            runtime_for_max.update(cx, |runtime, _| {
                                runtime.max_text = next_max.clone();
                            });
                            if let Some(on_change) = on_change_for_max.as_ref() {
                                on_change(
                                    TableRangeFilterChange::new(
                                        column_id_for_max.clone(),
                                        runtime_for_max.read(cx).min_text.clone(),
                                        next_max,
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
        )
        .when(clear_enabled, |this| {
            let runtime_for_clear = runtime.clone();
            let on_change_for_clear = on_change.clone();
            let column_id_for_clear = column_id.clone();
            this.child(
                div().flex().justify_end().child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.min_text.clear();
                                runtime.max_text.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(
                                    TableRangeFilterChange::clear(column_id_for_clear.clone()),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
            )
        })
}
