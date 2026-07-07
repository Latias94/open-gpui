use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeContext;
use open_gpui::prelude::*;
use open_gpui::{
    Entity, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px, rgba,
};
use open_gpui_ui_core::{Sizable, Size, TableColumnVisibilityOverrides, Toggled, UiPx};

use super::TableColumnVisibilityChangeHandler;
use super::component::TableColumnVisibilityRuntime;
use super::state::{
    TableColumnVisibilityChange, TableColumnVisibilityItemState, TableColumnVisibilityState,
};

pub(in crate::table::column_visibility) fn table_column_visibility_content_element(
    content_id: String,
    items_id: String,
    state: TableColumnVisibilityState,
    runtime: Entity<TableColumnVisibilityRuntime>,
    on_change: Option<TableColumnVisibilityChangeHandler>,
    items_height: UiPx,
    size: Size,
    theme: &ThemeContext,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let content_debug_id = state.id().to_owned();
    let count_text = format!("{}/{} visible", state.visible_count(), state.item_count());
    let items = state.items().to_vec();
    let hideable_column_ids = state
        .items()
        .iter()
        .filter(|item| item.hideable())
        .map(|item| item.column_id().clone())
        .collect::<Vec<_>>();
    let show_all_enabled = state.show_all_enabled();
    let reset_enabled = state.reset_enabled();
    let show_all_label = state.show_all_label().to_owned();
    let reset_label = state.reset_label().to_owned();
    let empty_label = state.empty_label().to_owned();
    let show_all_debug_id = state.id().to_owned();
    let reset_debug_id = state.id().to_owned();
    let runtime_for_show_all = runtime.clone();
    let runtime_for_reset = runtime.clone();
    let on_change_for_show_all = on_change.clone();
    let on_change_for_reset = on_change.clone();
    let show_all_ids = hideable_column_ids.clone();
    let show_all_change_ids = hideable_column_ids;
    let body = if state.empty() {
        div()
            .min_w(px(0.0))
            .py(px(4.0))
            .text_sm()
            .opacity(0.72)
            .child(empty_label)
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_h(px(0.0))
            .h(gpui_px_from_ui(items_height))
            .overflow_hidden()
            .child(
                ScrollArea::new(
                    items_id,
                    table_column_visibility_items_element(
                        state.clone(),
                        items,
                        runtime,
                        on_change,
                        disabled,
                    ),
                )
                .vertical()
                .with_size(size),
            )
            .into_any_element()
    };

    div()
        .id(content_id)
        .debug_selector(move || format!("table-column-visibility:{content_debug_id}:content"))
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
                .child(div().flex_none().text_xs().opacity(0.72).child(count_text)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .debug_selector(move || {
                            format!("table-column-visibility:{show_all_debug_id}:show-all")
                        })
                        .child(
                            Button::new(format!("{}-show-all", state.id()), show_all_label)
                                .variant(ButtonVariant::Ghost)
                                .with_size(size)
                                .disabled(disabled || !show_all_enabled)
                                .on_click(move |_, window, cx| {
                                    runtime_for_show_all.update(cx, |runtime, _| {
                                        runtime.visibility = show_all_ids.iter().cloned().fold(
                                            runtime.visibility.clone(),
                                            |visibility, column_id| {
                                                visibility.with_visibility(column_id, true)
                                            },
                                        );
                                    });
                                    if let Some(on_change) = on_change_for_show_all.as_ref() {
                                        on_change(
                                            TableColumnVisibilityChange::show_all(
                                                show_all_change_ids.clone(),
                                            ),
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .debug_selector(move || {
                            format!("table-column-visibility:{reset_debug_id}:reset")
                        })
                        .child(
                            Button::new(format!("{}-reset", state.id()), reset_label)
                                .variant(ButtonVariant::Ghost)
                                .with_size(size)
                                .disabled(disabled || !reset_enabled)
                                .on_click(move |_, window, cx| {
                                    runtime_for_reset.update(cx, |runtime, _| {
                                        runtime.visibility =
                                            TableColumnVisibilityOverrides::default();
                                    });
                                    if let Some(on_change) = on_change_for_reset.as_ref() {
                                        on_change(TableColumnVisibilityChange::reset(), window, cx);
                                    }
                                }),
                        ),
                ),
        )
        .child(body)
}

fn table_column_visibility_items_element(
    state: TableColumnVisibilityState,
    items: Vec<TableColumnVisibilityItemState>,
    runtime: Entity<TableColumnVisibilityRuntime>,
    on_change: Option<TableColumnVisibilityChangeHandler>,
    disabled: bool,
) -> impl IntoElement {
    items.into_iter().fold(
        div().flex().flex_col().gap_1().min_w(px(0.0)),
        |list, item| {
            let column_id = item.column_id().clone();
            let column_id_for_checkbox = column_id.clone();
            let column_id_text = column_id.as_str().to_owned();
            let column_id_text_for_row = column_id_text.clone();
            let label = item.label().to_owned();
            let checked = item.checked();
            let row_disabled = disabled || item.disabled();
            let next_checked = !checked;
            let runtime_for_row = runtime.clone();
            let runtime_for_checkbox = runtime.clone();
            let on_change_for_row = on_change.clone();
            let on_change_for_checkbox = on_change.clone();
            let column_id_for_row = column_id.clone();
            let debug_id = state.id().to_owned();
            let row_id = format!("{}-column-row-{column_id_text}", state.id());
            let checkbox_id = format!("{}-column-{column_id_text}", state.id());

            list.child(
                div()
                    .id(row_id)
                    .debug_selector(move || {
                        format!(
                            "table-column-visibility:{debug_id}:column:{column_id_text_for_row}"
                        )
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded(px(6.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .when(row_disabled, |this| this.opacity(0.56))
                    .when(!row_disabled, move |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(rgba(0x00000010)))
                            .on_click(move |_, window, cx| {
                                runtime_for_row.update(cx, |runtime, _| {
                                    runtime.visibility = runtime
                                        .visibility
                                        .clone()
                                        .with_visibility(column_id_for_row.clone(), next_checked);
                                });
                                if let Some(on_change) = on_change_for_row.as_ref() {
                                    on_change(
                                        TableColumnVisibilityChange::new(
                                            column_id_for_row.clone(),
                                            next_checked,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })
                    })
                    .child(
                        Checkbox::new(checkbox_id)
                            .label(label)
                            .checked(checked)
                            .disabled(row_disabled)
                            .on_toggle(move |toggled, _event, window, cx| {
                                let next_visible = matches!(toggled, Toggled::True);
                                runtime_for_checkbox.update(cx, |runtime, _| {
                                    runtime.visibility =
                                        runtime.visibility.clone().with_visibility(
                                            column_id_for_checkbox.clone(),
                                            next_visible,
                                        );
                                });
                                if let Some(on_change) = on_change_for_checkbox.as_ref() {
                                    on_change(
                                        TableColumnVisibilityChange::new(
                                            column_id_for_checkbox.clone(),
                                            next_visible,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .opacity(0.72)
                            .child(if row_disabled {
                                "Locked".to_string()
                            } else if checked {
                                "Visible".to_string()
                            } else {
                                "Hidden".to_string()
                            }),
                    ),
            )
        },
    )
}
