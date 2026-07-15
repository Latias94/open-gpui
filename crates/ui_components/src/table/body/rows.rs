use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, Axis, ClickEvent, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Pixels, StatefulInteractiveElement, Styled, div, px, rgb,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, TableColumnRegion, TableRowRegion, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::scroll_area::ScrollAreaMetrics;
use crate::table::body::keyboard::TableKeyboardDispatchContext;
use crate::table::body::layout::{render_table_lane_spacer, table_row_region_cells_for_window};
use crate::table::cell::render_table_body_cell;
use crate::table::identity::{TableDebugSelector, table_row_element_id};
use crate::table::interaction::request_table_row_selection_change;
use crate::table::runtime::TableRuntimeRenderSnapshot;
use crate::table::scroll::render_table_scroll_viewport;
use crate::table::{
    TableCenterColumnWindowPlan, TableInputModifiers, TableRowAction, TableRowActivation,
    TableRowActivationKind, TableRowRenderPlan, TableSelectionScope,
};

use super::{TableBodyRenderContext, TableRowRenderContext};

pub(in crate::table::body) fn render_table_row_band(
    context: Rc<TableBodyRenderContext>,
    runtime_snapshot: &TableRuntimeRenderSnapshot,
    region: TableRowRegion,
    rows: Vec<TableRowRenderPlan>,
    height: UiPx,
) -> AnyElement {
    let table_id = context.table_id.clone();
    let body_selector = TableDebugSelector::body_region(&table_id, region);
    div()
        .debug_selector(move || body_selector.clone())
        .relative()
        .w_full()
        .h(gpui_px_from_ui(height))
        .flex_none()
        .children(rows.into_iter().map(move |row| {
            let focus_handle = runtime_snapshot.focus_handle(row.identity());
            let focused = runtime_snapshot.is_focused(row.identity());
            render_table_row(TableRowRenderContext {
                body: context.clone(),
                row: Rc::new(row),
                focus_handle,
                focused,
            })
        }))
        .into_any_element()
}

fn render_table_row(context: TableRowRenderContext) -> impl IntoElement {
    let row = context.row.as_ref();
    let table_id = context.body.table_id.clone();
    let metrics = context.body.metrics;
    let measured_rows = context.body.measured_rows;
    let center_window = context.body.center_window.clone();
    let row_identity_key = row.row().identity_key().clone();
    let tree_branch = row.is_tree_branch();
    let tree_expanded = row.tree_expanded().unwrap_or(false);
    let virtual_size = row.virtual_size();
    let row_background = if row.row().is_group() {
        rgb(0xf1f4f8)
    } else if row.selected() {
        rgb(0xe7f0ff)
    } else if row.model_index().is_multiple_of(2) {
        rgb(0xffffff)
    } else {
        rgb(0xf8f9f3)
    };
    let region_cells = TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            let source_cells = row.cells_for_region(region).cloned().collect::<Vec<_>>();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let cells = table_row_region_cells_for_window(&source_cells, active_center_window);
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| {
                    source_cells
                        .iter()
                        .fold(UiPx::ZERO, |total, cell| total + cell.width())
                });
            let leading_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::leading_spacer_width)
                .unwrap_or(UiPx::ZERO);
            let trailing_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::trailing_spacer_width)
                .unwrap_or(UiPx::ZERO);
            (
                region,
                region_width,
                cells,
                !source_cells.is_empty(),
                leading_spacer_width,
                trailing_spacer_width,
                active_center_window.is_some(),
            )
        })
        .collect::<Vec<_>>();
    let tree_affordance_column_id = row.row().tree().and_then(|_| {
        region_cells.iter().find_map(|(_, _, cells, _, _, _, _)| {
            cells.first().map(|cell| cell.column_id().clone())
        })
    });
    let mut semantics = SemanticDescriptor::new(Role::Row)
        .with_row_index(row.aria_row_index())
        .with_selected(row.selected())
        .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);
    if tree_branch {
        semantics = semantics.with_expanded(tree_expanded);
    }

    let horizontal_scrollbar_width =
        gpui_px_from_ui(ScrollAreaMetrics::from_size(metrics.size()).scrollbar_width());
    div()
        .on_children_prepainted({
            let context = context.clone();
            move |row_bounds, _window, cx| {
                if context.body.measured_rows {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max)
                        .ceil();
                    context.body.runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            context.row.identity().clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
        .id(table_row_element_id(&table_id, &row_identity_key))
        .debug_selector({
            let table_id = table_id.clone();
            let row_identity_key = row_identity_key.clone();
            move || TableDebugSelector::row_key(&table_id, &row_identity_key)
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .min_w(px(0.0))
        .flex()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .hover(|this| this.bg(rgb(0xeef2f7)))
        .ui_semantics(&semantics)
        .focusable()
        .tab_stop(context.focused)
        .when_some(context.focus_handle.clone(), {
            let context = context.clone();
            move |this, focus_handle| {
                let action_focus_handle = focus_handle.clone();
                this.track_focus(&focus_handle).on_ui_a11y_action(
                    AccessibleAction::Focus,
                    move |_, window, cx| {
                        context.body.runtime.update(cx, |runtime, cx| {
                            runtime.set_focused(context.row.identity().clone(), cx);
                        });
                        action_focus_handle.focus(window, cx);
                    },
                )
            }
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(
            !tree_branch || context.body.on_row_activate.is_some(),
            |this| this.cursor_pointer(),
        )
        .on_click({
            let context = context.clone();
            move |event: &ClickEvent, window, cx| {
                if !event.standard_click() || window.default_prevented() {
                    return;
                }

                cx.stop_propagation();
                window.prevent_default();

                let action = TableRowAction::from_render_plan(
                    context.row.as_ref(),
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                let selection_policy = context.body.selection_policy;
                if selection_policy.activation_mode().is_row_click() {
                    request_table_row_selection_change(
                        &action,
                        selection_policy,
                        TableSelectionScope::Row,
                        context.body.selected_row_ids.clone(),
                        context.body.on_row_selection_change.clone(),
                        window,
                        cx,
                    );
                }

                let activation_kind = if event.click_count() >= 2 {
                    TableRowActivationKind::DoubleClick
                } else {
                    TableRowActivationKind::Click
                };
                context.body.runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(context.row.identity().clone(), cx);
                });
                if let Some(focus_handle) = context.focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                if let Some(on_row_activate) = context.body.on_row_activate.as_ref() {
                    on_row_activate(TableRowActivation::new(action, activation_kind), window, cx);
                }
            }
        })
        .on_key_down({
            let context = context.clone();
            move |event: &KeyDownEvent, window, cx| {
                let Some(focus_handle) = context.focus_handle.as_ref() else {
                    return;
                };
                TableKeyboardDispatchContext {
                    final_model: context.body.resolved_table.final_model(),
                    vertical_scroll_handle: context.body.vertical_scroll_handle.clone(),
                    top_row_count: context.body.top_row_count,
                    center_total_row_count: context.body.center_total_row_count,
                    fallback_row_height: metrics.row_height(),
                    fallback_viewport_extent: metrics.viewport_extent(),
                    runtime: &context.body.runtime,
                    current_expansion: context.body.current_expansion.clone(),
                    on_row_activate: context.body.on_row_activate.clone(),
                    on_row_expansion_request: context.body.on_row_expansion_request.clone(),
                }
                .dispatch_rendered_row(
                    context.row.as_ref(),
                    focus_handle,
                    event,
                    window,
                    cx,
                );
            }
        })
        .children(region_cells.into_iter().map({
            let context = context.clone();
            move |(
                region,
                region_width,
                cells,
                has_source_cells,
                leading_spacer_width,
                trailing_spacer_width,
                uses_center_window,
            )| {
                let table_id = context.body.table_id.clone();
                let row_identity_key = context.row.row().identity_key().clone();
                let center_scroll_selector = (context.body.has_pinned_columns
                    && region == TableColumnRegion::Center
                    && has_source_cells)
                    .then(|| {
                        TableDebugSelector::row_center_scroll_key(&table_id, &row_identity_key)
                    });
                let mut region_children =
                    Vec::with_capacity(cells.len() + usize::from(uses_center_window) * 2);
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(leading_spacer_width));
                }
                region_children.extend(cells.into_iter().map({
                    let context = context.clone();
                    let tree_affordance_column_id = tree_affordance_column_id.clone();
                    move |cell| {
                        let tree_affordance = tree_affordance_column_id
                            .as_ref()
                            .is_some_and(|column_id| cell.column_id() == column_id);
                        render_table_body_cell(context.clone(), cell, tree_affordance)
                            .into_any_element()
                    }
                }));
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(trailing_spacer_width));
                }

                let mut region_lane = div()
                    .min_w(px(0.0))
                    .flex()
                    .overflow_hidden()
                    .debug_selector({
                        let selector = TableDebugSelector::row_region_key(
                            &table_id,
                            &row_identity_key,
                            region,
                        );
                        move || selector.clone()
                    })
                    .w(gpui_px_from_ui(region_width))
                    .flex_none()
                    .children(region_children);

                region_lane = if measured_rows {
                    region_lane.items_start()
                } else {
                    region_lane.h_full().items_center()
                };

                let region_lane = region_lane.into_any_element();

                if let Some(center_scroll_selector) = center_scroll_selector {
                    render_table_scroll_viewport(
                        center_scroll_selector,
                        Axis::Horizontal,
                        horizontal_scrollbar_width,
                        &context.body.horizontal_scroll_handle,
                        region_lane,
                    )
                } else {
                    region_lane
                }
            }
        }))
        .when(!measured_rows, |this| this.h(gpui_px_from_ui(virtual_size)))
        .into_any_element()
}
