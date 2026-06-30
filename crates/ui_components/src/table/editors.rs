use open_gpui::prelude::*;
use open_gpui::{AnyElement, ParentElement, Styled, div, px};
use open_gpui_ui_core::{Sizable, TableCellEditor, TableCellValue, Toggled};

use crate::checkbox::Checkbox;
use crate::listbox::ListboxOption;
use crate::select::Select;
use crate::text_input::TextInput;
use crate::textarea::Textarea;

use super::{
    TableCellEditChange, TableCellEditHandler, TableCellRenderPlan, TableInputModifiers,
    TableMetrics, TableRowAction, TableRowRenderPlan,
};

pub(super) fn render_table_cell_editor(
    table_id: &str,
    render_key: &str,
    column_id: &str,
    metrics: TableMetrics,
    cell: &TableCellRenderPlan,
    row: &TableRowRenderPlan,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> Option<AnyElement> {
    let (Some(editor), Some(_)) = (cell.editor(), on_cell_edit_change.as_ref()) else {
        return None;
    };

    let action = TableRowAction::from_render_plan(row, TableInputModifiers::default());
    let column_id_for_change = cell.column_id().clone();
    let previous_value = cell.value().cloned().unwrap_or_default();
    let cell_text = cell.text().to_owned();
    let cell_value = cell.value().cloned();
    let select_options = cell
        .select_options()
        .iter()
        .map(|option| ListboxOption::new(option.value().to_owned(), option.label().to_owned()))
        .collect::<Vec<_>>();
    let selected_value = cell_value
        .as_ref()
        .map(TableCellValue::filter_text)
        .unwrap_or_default();
    let editor_id = format!("table:{table_id}:cell:{render_key}:{column_id}:editor");
    let editor_label = format!("Edit {column_id} for row {}", row.id().as_str());
    let editor_element = match editor {
        TableCellEditor::Text => {
            let on_change = on_cell_edit_change.clone();
            TextInput::new(editor_id, editor_label)
                .value(cell_text)
                .on_change(move |next_text, window, cx| {
                    if let Some(on_change) = on_change.as_ref() {
                        on_change(
                            TableCellEditChange::new(
                                action.clone(),
                                column_id_for_change.clone(),
                                previous_value.clone(),
                                next_text,
                            ),
                            window,
                            cx,
                        );
                    }
                })
                .with_size(metrics.size())
                .into_any_element()
        }
        TableCellEditor::MultilineText { rows } => {
            let on_change = on_cell_edit_change.clone();
            Textarea::new(editor_id, editor_label)
                .value(cell_text)
                .rows(rows)
                .on_change(move |next_text, window, cx| {
                    if let Some(on_change) = on_change.as_ref() {
                        on_change(
                            TableCellEditChange::new(
                                action.clone(),
                                column_id_for_change.clone(),
                                previous_value.clone(),
                                next_text,
                            ),
                            window,
                            cx,
                        );
                    }
                })
                .with_size(metrics.size())
                .into_any_element()
        }
        TableCellEditor::Checkbox => {
            let on_change = on_cell_edit_change.clone();
            let checked = matches!(cell_value.as_ref(), Some(TableCellValue::Bool(true)));
            let editor_label = format!("Toggle {column_id} for row {}", row.id().as_str());
            Checkbox::new(editor_id)
                .aria_label(editor_label)
                .checked(checked)
                .on_toggle(move |next_toggled, _, window, cx| {
                    if let Some(on_change) = on_change.as_ref() {
                        on_change(
                            TableCellEditChange::new(
                                action.clone(),
                                column_id_for_change.clone(),
                                previous_value.clone(),
                                matches!(next_toggled, Toggled::True),
                            ),
                            window,
                            cx,
                        );
                    }
                })
                .into_any_element()
        }
        TableCellEditor::Select => {
            let on_change = on_cell_edit_change.clone();
            Select::new(editor_id, editor_label)
                .full_width(true)
                .placeholder(cell_text.clone())
                .selected(selected_value)
                .options(select_options)
                .on_select(move |selection, window, cx| {
                    if let Some(on_change) = on_change.as_ref() {
                        on_change(
                            TableCellEditChange::new(
                                action.clone(),
                                column_id_for_change.clone(),
                                previous_value.clone(),
                                TableCellValue::Text(selection.value().to_owned()),
                            ),
                            window,
                            cx,
                        );
                    }
                })
                .into_any_element()
        }
    };

    Some(
        div()
            .id(format!(
                "table:{table_id}:cell:{render_key}:{column_id}:editor-shell"
            ))
            .debug_selector({
                let table_id = table_id.to_owned();
                let render_key = render_key.to_owned();
                let column_id = column_id.to_owned();
                move || format!("table:{table_id}:cell:{render_key}:{column_id}:editor-shell")
            })
            .flex_1()
            .w_full()
            .min_w(px(0.0))
            .overflow_hidden()
            .block_mouse_except_scroll()
            .when(matches!(editor, TableCellEditor::Checkbox), |this| {
                this.flex().justify_center().items_center()
            })
            .child(editor_element)
            .into_any_element(),
    )
}
