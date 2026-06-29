use std::rc::Rc;

mod keyboard;
mod layout;
mod scroll;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, ScrollHandle, StatefulInteractiveElement, Styled, div, px, rgb,
};
use open_gpui_ui_core::{
    Sizable, TableColumnRegion, TableExpansionState, TableResolvedRow, TableRowId, TableRowRegion,
    TableSelectionPolicy, TableTreeRow, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::scroll_area::ScrollArea;

use super::cell::render_table_body_cell;
use super::interaction::request_table_row_selection_change;
use super::runtime::TableRuntime;
use super::virtualization::table_rows_virtual_size;
use super::{
    TableCellEditHandler, TableCenterColumnWindowPlan, TableInputModifiers, TableMetrics,
    TablePinnedLayoutPlan, TableRenderPlan, TableRowAction, TableRowActivation,
    TableRowActivationHandler, TableRowActivationKind, TableRowExpansionHandler,
    TableRowRenderPlan, TableRowSelectionHandler, TableSelectionScope,
};

use keyboard::handle_table_row_key_down;
use layout::{render_table_lane_spacer, table_row_region_cells_for_window};

pub(super) use scroll::handle_table_vertical_scroll_wheel;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_table_body(
    plan: &TableRenderPlan,
    scroll_viewport_id: String,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    header_band_height: UiPx,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let final_rows = Rc::new(plan.table().final_model().rows().to_vec());
    let top_rows = plan.top_rows().to_vec();
    let center_rows = plan.rows().to_vec();
    let bottom_rows = plan.bottom_rows().to_vec();
    let top_row_count = top_rows.len();
    let center_total_row_count = plan.virtualizer().count();
    let top_height = table_rows_virtual_size(&top_rows);
    let center_height = plan.virtualizer().total_size();
    let bottom_height = table_rows_virtual_size(&bottom_rows);
    let measured_rows = plan.row_measure_mode().measured();

    div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .pt(gpui_px_from_ui(header_band_height))
        .flex()
        .flex_col()
        .when(!top_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Top,
                metrics,
                top_rows.clone(),
                top_height,
                pinned_layout.clone(),
                center_window.clone(),
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                runtime_snapshot.clone(),
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
                measured_rows,
            ))
        })
        .child(
            div().flex_1().min_h(px(0.0)).overflow_hidden().child(
                ScrollArea::new(
                    scroll_viewport_id,
                    render_table_row_band(
                        &table_id,
                        TableRowRegion::Center,
                        metrics,
                        center_rows,
                        center_height,
                        pinned_layout.clone(),
                        center_window.clone(),
                        horizontal_scroll_handle.clone(),
                        vertical_scroll_handle.clone(),
                        runtime.clone(),
                        runtime_snapshot.clone(),
                        final_rows.clone(),
                        top_row_count,
                        center_total_row_count,
                        current_expansion.clone(),
                        selection_policy,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        on_row_activate.clone(),
                        on_row_expansion_request.clone(),
                        on_cell_edit_change.clone(),
                        measured_rows,
                    ),
                )
                .vertical()
                .scroll_handle(&vertical_scroll_handle)
                .with_size(metrics.size()),
            ),
        )
        .when(!bottom_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Bottom,
                metrics,
                bottom_rows.clone(),
                bottom_height,
                pinned_layout,
                center_window,
                horizontal_scroll_handle,
                vertical_scroll_handle,
                runtime,
                runtime_snapshot,
                final_rows,
                top_row_count,
                center_total_row_count,
                current_expansion,
                selection_policy,
                selected_row_ids,
                on_row_selection_change,
                on_row_activate,
                on_row_expansion_request,
                on_cell_edit_change,
                measured_rows,
            ))
        })
}

#[allow(clippy::too_many_arguments)]
fn render_table_row_band(
    table_id: &str,
    region: TableRowRegion,
    metrics: TableMetrics,
    rows: Vec<TableRowRenderPlan>,
    height: UiPx,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> AnyElement {
    let table_id = table_id.to_owned();
    let region_name = region.as_str();
    div()
        .id(format!("table:{table_id}:body:{region_name}"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:body:{region_name}")
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(height))
        .flex_none()
        .children(rows.into_iter().map(move |row| {
            let table_id = table_id.clone();
            let center_window = center_window.clone();
            let focus_handle = runtime_snapshot.focus_handles.get(row.id()).cloned();
            let focused = runtime_snapshot.focused_row.as_ref() == Some(row.id());
            render_table_row(
                table_id,
                row,
                metrics,
                pinned_layout.clone(),
                center_window,
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                focus_handle,
                focused,
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
                measured_rows,
            )
        }))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_table_row(
    table_id: String,
    row: TableRowRenderPlan,
    metrics: TableMetrics,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    focused: bool,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_id = row.id().clone();
    let row_for_layout = row.clone();
    let row_for_click = row.clone();
    let row_for_key = row.clone();
    let tree = row.row().tree().cloned();
    let tree_depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
    let tree_branch = row.row().is_tree_branch();
    let tree_expanded = row.row().tree_expanded().unwrap_or(false);
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
    let tree_affordance_column_id = tree.as_ref().and_then(|_| {
        region_cells.iter().find_map(|(_, _, cells, _, _, _, _)| {
            cells.first().map(|cell| cell.column_id().clone())
        })
    });

    let row_element = div()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let row_key = render_key.clone();
            move |row_bounds, _window, cx| {
                if measured_rows {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max);
                    let measured_height = measured_height.ceil();
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            row_key.clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
        .id(format!("table:{table_id}:row:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let render_key = render_key.clone();
            move || format!("table:{table_id}:row:{render_key}")
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
        .ui_role(row.role())
        .aria_row_index(row.aria_row_index())
        .aria_selected(row.selected())
        .when(tree_branch, |this| this.aria_expanded(tree_expanded))
        .focusable()
        .tab_stop(focused)
        .when_some(focus_handle.clone(), |this, focus_handle| {
            this.track_focus(&focus_handle)
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(!tree_branch || on_row_activate.is_some(), |this| {
            this.cursor_pointer()
        })
        .on_click({
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let selection_policy = selection_policy;
            let selected_row_ids = selected_row_ids.clone();
            let on_row_selection_change = on_row_selection_change.clone();
            let on_row_activate = on_row_activate.clone();
            move |event: &ClickEvent, window, cx| {
                if !event.standard_click() || window.default_prevented() {
                    return;
                }

                cx.stop_propagation();
                window.prevent_default();

                let action = TableRowAction::from_render_plan(
                    &row_for_click,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                if selection_policy.activation_mode().is_row_click() {
                    request_table_row_selection_change(
                        &runtime,
                        &action,
                        selection_policy,
                        TableSelectionScope::Row,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        window,
                        cx,
                    );
                }

                let activation_kind = if event.click_count() >= 2 {
                    TableRowActivationKind::DoubleClick
                } else {
                    TableRowActivationKind::Click
                };
                runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(row_id.clone(), cx);
                });
                if let Some(focus_handle) = focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                if let Some(on_row_activate) = on_row_activate.as_ref() {
                    on_row_activate(TableRowActivation::new(action, activation_kind), window, cx);
                }
            }
        })
        .on_key_down({
            let runtime = runtime.clone();
            let on_row_activate = on_row_activate.clone();
            let on_row_expansion_request = on_row_expansion_request.clone();
            let current_expansion_for_key = current_expansion.clone();
            move |event: &KeyDownEvent, window, cx| {
                handle_table_row_key_down(
                    &row_for_key,
                    final_rows.as_ref(),
                    vertical_scroll_handle.clone(),
                    top_row_count,
                    center_total_row_count,
                    &runtime,
                    current_expansion_for_key.clone(),
                    on_row_activate.clone(),
                    on_row_expansion_request.clone(),
                    event,
                    window,
                    cx,
                );
            }
        })
        .children(region_cells.into_iter().map(
            move |(
                region,
                region_width,
                cells,
                has_source_cells,
                leading_spacer_width,
                trailing_spacer_width,
                uses_center_window,
            )| {
                let table_id = table_id.clone();
                let render_key = render_key.clone();
                let region_name = region.as_str().to_owned();
                let center_scroll_id = pinned_layout.as_ref().and_then(|layout| {
                    (region == TableColumnRegion::Center && has_source_cells)
                        .then(|| layout.row_center_scroll_id(&render_key))
                });
                let mut region_children =
                    Vec::with_capacity(cells.len() + usize::from(uses_center_window) * 2);
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(leading_spacer_width));
                }
                let current_expansion_for_cells = current_expansion.clone();
                region_children.extend(cells.into_iter().map({
                    let table_id = table_id.clone();
                    let render_key = render_key.clone();
                    let row = row.clone();
                    let runtime = runtime.clone();
                    let focus_handle = focus_handle.clone();
                    let on_row_expansion_request = on_row_expansion_request.clone();
                    let on_cell_edit_change = on_cell_edit_change.clone();
                    let tree = tree.clone();
                    let tree_affordance_column_id = tree_affordance_column_id.clone();
                    move |cell| {
                        let tree_affordance = tree_affordance_column_id
                            .as_ref()
                            .is_some_and(|column_id| cell.column_id() == column_id);
                        render_table_body_cell(
                            table_id.clone(),
                            render_key.clone(),
                            metrics,
                            cell,
                            row.clone(),
                            tree.clone(),
                            tree_depth,
                            tree_branch,
                            tree_expanded,
                            tree_affordance,
                            runtime.clone(),
                            focus_handle.clone(),
                            current_expansion_for_cells.clone(),
                            on_row_expansion_request.clone(),
                            on_cell_edit_change.clone(),
                            measured_rows,
                        )
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
                    .id(format!(
                        "table:{table_id}:row-region:{render_key}:{region_name}"
                    ))
                    .debug_selector({
                        let table_id = table_id.clone();
                        let render_key = render_key.clone();
                        let region_name = region_name.clone();
                        move || format!("table:{table_id}:row-region:{render_key}:{region_name}")
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

                if let Some(center_scroll_id) = center_scroll_id {
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .child(
                            ScrollArea::new(center_scroll_id, region_lane)
                                .horizontal()
                                .scroll_handle(&horizontal_scroll_handle)
                                .with_size(metrics.size()),
                        )
                        .into_any_element()
                } else {
                    region_lane
                }
            },
        ))
        .when(!measured_rows, |this| {
            this.h(gpui_px_from_ui(row_for_layout.virtual_size()))
        })
        .into_any_element();
    row_element
}
