use std::rc::Rc;

mod keyboard;
mod layout;
mod rows;

pub(super) use keyboard::TableKeyboardDispatchContext;

use open_gpui::prelude::*;
use open_gpui::{
    Axis, Entity, FocusHandle, IntoElement, ParentElement, ScrollHandle, Styled, div, px,
};
use open_gpui_ui_core::{
    TableExpansionState, TableResolvedState, TableRowId, TableRowRegion, TableSelectionPolicy, UiPx,
};

use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollAreaMetrics;

use super::identity::TableDebugSelector;
use super::runtime::{TableRuntime, TableRuntimeRenderSnapshot};
use super::scroll::render_table_scroll_viewport;
use super::virtualization::table_rows_virtual_size;
use super::{
    TableCellEditHandler, TableCenterColumnWindowPlan, TableMetrics, TableRenderPlan,
    TableRowActivationHandler, TableRowExpansionHandler, TableRowRenderPlan,
    TableRowSelectionHandler,
};

use rows::render_table_row_band;

pub(super) struct TableBodyRenderInput {
    pub(super) horizontal_scroll_handle: ScrollHandle,
    pub(super) vertical_scroll_handle: ScrollHandle,
    pub(super) runtime: Entity<TableRuntime>,
    pub(super) runtime_snapshot: Rc<TableRuntimeRenderSnapshot>,
    pub(super) current_expansion: TableExpansionState,
    pub(super) selection_policy: TableSelectionPolicy,
    pub(super) selected_row_ids: Rc<Vec<TableRowId>>,
    pub(super) on_row_selection_change: Option<TableRowSelectionHandler>,
    pub(super) on_row_activate: Option<TableRowActivationHandler>,
    pub(super) on_row_expansion_request: Option<TableRowExpansionHandler>,
    pub(super) on_cell_edit_change: Option<TableCellEditHandler>,
}

pub(super) struct TableBodyRenderContext {
    pub(super) table_id: String,
    pub(super) metrics: TableMetrics,
    pub(super) has_pinned_columns: bool,
    pub(super) center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    pub(super) horizontal_scroll_handle: ScrollHandle,
    pub(super) vertical_scroll_handle: ScrollHandle,
    pub(super) runtime: Entity<TableRuntime>,
    pub(super) resolved_table: Rc<TableResolvedState>,
    pub(super) top_row_count: usize,
    pub(super) center_total_row_count: usize,
    pub(super) current_expansion: TableExpansionState,
    pub(super) selection_policy: TableSelectionPolicy,
    pub(super) selected_row_ids: Rc<Vec<TableRowId>>,
    pub(super) on_row_selection_change: Option<TableRowSelectionHandler>,
    pub(super) on_row_activate: Option<TableRowActivationHandler>,
    pub(super) on_row_expansion_request: Option<TableRowExpansionHandler>,
    pub(super) on_cell_edit_change: Option<TableCellEditHandler>,
    pub(super) measured_rows: bool,
}

#[derive(Clone)]
pub(super) struct TableRowRenderContext {
    pub(super) body: Rc<TableBodyRenderContext>,
    pub(super) row: Rc<TableRowRenderPlan>,
    pub(super) focus_handle: Option<FocusHandle>,
    pub(super) focused: bool,
}

pub(super) fn render_table_body(
    plan: &TableRenderPlan,
    header_band_height: UiPx,
    input: TableBodyRenderInput,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let has_pinned_columns = plan.has_pinned_columns();
    let center_window = if has_pinned_columns {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let resolved_table = plan.resolved_table();
    let top_rows = plan.top_rows().to_vec();
    let center_rows = plan.rows().to_vec();
    let bottom_rows = plan.bottom_rows().to_vec();
    let top_row_count = top_rows.len();
    let has_top_rows = !top_rows.is_empty();
    let has_bottom_rows = !bottom_rows.is_empty();
    let center_total_row_count = plan.virtualizer().count();
    let top_height = table_rows_virtual_size(&top_rows);
    let center_height = plan.virtualizer().total_size();
    let bottom_height = table_rows_virtual_size(&bottom_rows);
    let measured_rows = plan.row_measure_mode().measured();
    let body_scroll_selector = TableDebugSelector::body_scroll(&table_id);
    let scrollbar_width =
        gpui_px_from_ui(ScrollAreaMetrics::from_size(metrics.size()).scrollbar_width());
    let runtime_snapshot = input.runtime_snapshot;
    let context = Rc::new(TableBodyRenderContext {
        table_id,
        metrics,
        has_pinned_columns,
        center_window,
        horizontal_scroll_handle: input.horizontal_scroll_handle,
        vertical_scroll_handle: input.vertical_scroll_handle,
        runtime: input.runtime,
        resolved_table,
        top_row_count,
        center_total_row_count,
        current_expansion: input.current_expansion,
        selection_policy: input.selection_policy,
        selected_row_ids: input.selected_row_ids,
        on_row_selection_change: input.on_row_selection_change,
        on_row_activate: input.on_row_activate,
        on_row_expansion_request: input.on_row_expansion_request,
        on_cell_edit_change: input.on_cell_edit_change,
        measured_rows,
    });

    div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .pt(gpui_px_from_ui(header_band_height))
        .flex()
        .flex_col()
        .when(has_top_rows, |this| {
            this.child(render_table_row_band(
                context.clone(),
                runtime_snapshot.as_ref(),
                TableRowRegion::Top,
                top_rows,
                top_height,
            ))
        })
        .child(render_table_scroll_viewport(
            body_scroll_selector,
            Axis::Vertical,
            scrollbar_width,
            &context.vertical_scroll_handle,
            render_table_row_band(
                context.clone(),
                runtime_snapshot.as_ref(),
                TableRowRegion::Center,
                center_rows,
                center_height,
            ),
        ))
        .when(has_bottom_rows, |this| {
            this.child(render_table_row_band(
                context,
                runtime_snapshot.as_ref(),
                TableRowRegion::Bottom,
                bottom_rows,
                bottom_height,
            ))
        })
}
