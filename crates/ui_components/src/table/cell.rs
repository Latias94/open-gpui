use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, ClickEvent, Entity, FocusHandle, IntoElement, ParentElement, Styled, Window, div,
    px, rgb,
};
use open_gpui_ui_core::{
    Role, TableExpansionState, TableRowChildrenLoadState, TableTreeRow, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::geometry::gpui_px_from_ui;

use super::editors::render_table_cell_editor;
use super::interaction::toggle_table_expansion;
use super::runtime::TableRuntime;
use super::{
    TableCellEditHandler, TableCellRenderPlan, TableInputModifiers, TableMetrics, TableRowAction,
    TableRowExpansionHandler, TableRowExpansionToggle, TableRowRenderPlan,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_table_body_cell(
    table_id: String,
    render_key: String,
    metrics: TableMetrics,
    cell: TableCellRenderPlan,
    row: TableRowRenderPlan,
    tree: Option<TableTreeRow>,
    tree_depth: usize,
    tree_branch: bool,
    tree_expanded: bool,
    tree_affordance: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> impl IntoElement {
    let column_id = cell.column_id().as_str().to_owned();
    let show_tree_affordance = tree_affordance && tree.is_some();
    let indent = ui_px(16.0) * tree_depth as f32;
    let mut content = Vec::new();
    if show_tree_affordance {
        content.push(
            div()
                .w(gpui_px_from_ui(indent))
                .when(!measured_rows, |this| this.h_full())
                .flex_none()
                .into_any_element(),
        );
        content.push(render_table_tree_toggle(
            table_id.clone(),
            render_key.clone(),
            row.clone(),
            tree_branch,
            tree_expanded,
            runtime,
            focus_handle,
            current_expansion,
            on_row_expansion_request,
        ));
    }
    if let Some(editor) = render_table_cell_editor(
        &table_id,
        &render_key,
        &column_id,
        metrics,
        &cell,
        &row,
        on_cell_edit_change,
    ) {
        content.push(editor);
    } else {
        let cell_text = cell.text().to_owned();
        content.push(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .when(measured_rows, |this| this.whitespace_normal())
                .when(!measured_rows, |this| this.truncate())
                .child(cell_text)
                .into_any_element(),
        );
    }

    let cell = div()
        .id(format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .debug_selector(move || format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .w(gpui_px_from_ui(cell.width()))
        .flex_none()
        .flex()
        .when(!measured_rows, |this| this.h_full().items_center())
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xe7e9e1))
        .text_xs()
        .text_color(rgb(0x2f3845))
        .ui_role(cell.role())
        .aria_column_index(cell.aria_column_index())
        .children(content)
        .when(measured_rows, |this| this.whitespace_normal())
        .when(!measured_rows, |this| this.truncate().whitespace_nowrap());

    cell.into_any_element()
}

fn render_table_tree_toggle(
    table_id: String,
    render_key: String,
    row: TableRowRenderPlan,
    tree_branch: bool,
    tree_expanded: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
) -> AnyElement {
    if !tree_branch {
        return div().w(px(18.0)).h(px(18.0)).flex_none().into_any_element();
    }

    let row_id = row.id().clone();
    let row_key = render_key.clone();
    let children_load_state = row
        .children_load_state()
        .cloned()
        .unwrap_or_else(TableRowChildrenLoadState::idle);
    let glyph = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => "...",
        TableRowChildrenLoadState::Failed { .. } => "!",
        TableRowChildrenLoadState::Idle if tree_expanded => "v",
        TableRowChildrenLoadState::Idle => ">",
    };
    let aria_label = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => {
            format!("Loading children for row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Failed { .. } => {
            format!("Retry loading row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle if tree_expanded => {
            format!("Collapse row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle => format!("Expand row {}", row.id().as_str()),
    };

    div()
        .id(format!("table:{table_id}:tree-toggle:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let row_key = row_key.clone();
            move || format!("table:{table_id}:tree-toggle:{row_key}")
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_role(Role::Button)
        .aria_label(aria_label)
        .aria_expanded(tree_expanded)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe8ede6)))
        .on_click(move |event: &ClickEvent, window: &mut Window, cx| {
            if !event.standard_click() || window.default_prevented() {
                return;
            }

            cx.stop_propagation();
            window.prevent_default();

            let next_expansion =
                toggle_table_expansion(current_expansion.clone(), row_id.clone(), !tree_expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row_id.clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(focus_handle) = focus_handle.as_ref() {
                focus_handle.focus(window, cx);
            }
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    &row,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, !tree_expanded),
                    window,
                    cx,
                );
            }
        })
        .child(glyph)
        .into_any_element()
}
