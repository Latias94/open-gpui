use open_gpui::prelude::*;
use open_gpui::{AnyElement, IntoElement, ParentElement, Styled, Window, div, px, rgb};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, TableRowChildrenLoadState, TableTreeRow, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::geometry::gpui_px_from_ui;

use super::editors::render_table_cell_editor;
use super::identity::{TableDebugSelector, table_cell_element_id, table_tree_toggle_element_id};
use super::{TableCellRenderPlan, TableInputModifiers, TableRowAction, TableRowExpansionToggle};
use crate::table::body::TableRowRenderContext;

pub(super) fn render_table_body_cell(
    context: TableRowRenderContext,
    cell: TableCellRenderPlan,
    tree_affordance: bool,
) -> impl IntoElement {
    let row = context.row.as_ref();
    let table_id = context.body.table_id.clone();
    let metrics = context.body.metrics;
    let measured_rows = context.body.measured_rows;
    let row_identity_key = row.row().identity_key().clone();
    let tree = row.row().tree();
    let show_tree_affordance = tree_affordance && tree.is_some();
    let tree_depth = tree.map(TableTreeRow::depth).unwrap_or(0);
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
        content.push(render_table_tree_toggle(context.clone()));
    }
    if let Some(editor) = render_table_cell_editor(
        &table_id,
        cell.column_id(),
        metrics,
        &cell,
        row,
        context.body.on_cell_edit_change.clone(),
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

    let semantics = SemanticDescriptor::new(Role::Cell)
        .with_value(cell.text())
        .with_column_index(cell.aria_column_index());
    let cell = div()
        .id(table_cell_element_id(
            &table_id,
            &row_identity_key,
            cell.column_id(),
        ))
        .debug_selector({
            let selector =
                TableDebugSelector::cell_key(&table_id, &row_identity_key, cell.column_id());
            move || selector.clone()
        })
        .w(gpui_px_from_ui(cell.width()))
        .flex_none()
        .flex()
        .when(!measured_rows, |this| this.h_full().items_center())
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xe7e9e1))
        .text_xs()
        .text_color(rgb(0x2f3845))
        .ui_semantics(&semantics)
        .children(content)
        .when(measured_rows, |this| this.whitespace_normal())
        .when(!measured_rows, |this| this.truncate().whitespace_nowrap());

    cell.into_any_element()
}

fn render_table_tree_toggle(context: TableRowRenderContext) -> AnyElement {
    if !context.row.is_tree_branch() {
        return div().w(px(18.0)).h(px(18.0)).flex_none().into_any_element();
    }

    let table_id = context.body.table_id.clone();
    let row_identity_key = context.row.row().identity_key().clone();
    let tree_expanded = context.row.tree_expanded().unwrap_or(false);
    let children_load_state = context
        .row
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
            format!(
                "Loading children for row {}",
                context.row.row().debug_label()
            )
        }
        TableRowChildrenLoadState::Failed { .. } => {
            format!("Retry loading row {}", context.row.row().debug_label())
        }
        TableRowChildrenLoadState::Idle if tree_expanded => {
            format!("Collapse row {}", context.row.row().debug_label())
        }
        TableRowChildrenLoadState::Idle => {
            format!("Expand row {}", context.row.row().debug_label())
        }
    };
    let semantics = SemanticDescriptor::new(Role::Button)
        .with_label(&aria_label)
        .with_expanded(tree_expanded)
        .with_actions(&[AccessibleAction::Click]);

    div()
        .id(table_tree_toggle_element_id(&table_id, &row_identity_key))
        .debug_selector({
            let selector = TableDebugSelector::tree_toggle_key(&table_id, &row_identity_key);
            move || selector.clone()
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_semantics(&semantics)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe8ede6)))
        .on_click(move |event, window: &mut Window, cx| {
            let event = event.window_event();
            if !event.standard_click() || window.default_prevented() {
                return;
            }

            cx.stop_propagation();
            window.prevent_default();

            let row_identity = context.row.identity().clone();
            context.body.runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row_identity.clone(), cx);
            });
            if let Some(focus_handle) = context.focus_handle.as_ref() {
                focus_handle.focus(window, cx);
            }
            if let Some(on_row_expansion_request) = context.body.on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    context.row.as_ref(),
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
