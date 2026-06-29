use open_gpui::prelude::*;
use open_gpui::{
    App, CursorStyle, DragMoveEvent, Empty, Entity, IntoElement, MouseButton, ParentElement,
    Pixels, Styled, Window, div, px, rgba,
};
use open_gpui_ui_core::{
    TableColumnId, TableColumnRegion, TableColumnResizeDirection, TableColumnResizeMode,
    TableColumnResizeState, TableColumnSizing, UiPx, drag_table_column_resize,
    end_table_column_resize,
};

use crate::geometry::ui_px_from_gpui;

use super::runtime::TableRuntime;
use super::{
    TableColumnOrderChange, TableColumnOrderHandler, TableColumnOrderPlacement,
    TableColumnRenderPlan, TableColumnSizingChange, TableColumnSizingHandler,
};

#[derive(Clone)]
pub(super) struct TableResizeRenderConfig {
    pub(super) table_id: String,
    pub(super) enabled: bool,
    pub(super) mode: TableColumnResizeMode,
    pub(super) direction: TableColumnResizeDirection,
    pub(super) base_sizing: TableColumnSizing,
    pub(super) runtime: Entity<TableRuntime>,
    pub(super) on_change: Option<TableColumnSizingHandler>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TableColumnResizeDrag {
    table_id: String,
    column_id: TableColumnId,
    start_width: UiPx,
    column_widths_start: Vec<(TableColumnId, UiPx)>,
    base_sizing: TableColumnSizing,
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TableColumnOrderDrag {
    pub(super) table_id: String,
    pub(super) column_id: TableColumnId,
    pub(super) region: TableColumnRegion,
}

pub(super) fn render_table_resize_handle(
    table_id: String,
    column: TableColumnRenderPlan,
    config: TableResizeRenderConfig,
) -> impl IntoElement {
    let column_id = column.id().clone();
    let column_key = column_id.as_str().to_owned();
    let drag = TableColumnResizeDrag {
        table_id: table_id.clone(),
        column_id: column_id.clone(),
        start_width: column.width(),
        column_widths_start: vec![(column_id.clone(), column.width())],
        base_sizing: config.base_sizing.clone(),
        mode: config.mode,
        direction: config.direction,
    };
    let drag_for_mouse_up = drag.clone();
    let drag_for_mouse_up_out = drag.clone();
    let drag_for_drag = drag.clone();
    let drag_table_id = table_id.clone();
    let drag_runtime = config.runtime.clone();
    let mouse_up_runtime = config.runtime.clone();
    let mouse_up_config = config.clone();
    let mouse_up_out_runtime = config.runtime.clone();
    let mouse_up_out_config = config;

    div()
        .id(format!("table:{table_id}:resize:{column_key}"))
        .debug_selector(move || format!("table:{table_id}:resize:{column_key}"))
        .absolute()
        .top(px(0.0))
        .right(px(0.0))
        .h_full()
        .w(px(10.0))
        .cursor(CursorStyle::ResizeColumn)
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_drag(
            drag_for_drag,
            move |drag, cursor_offset, bounds, window, cx| {
                if drag.table_id != drag_table_id {
                    return cx.new(|_| Empty);
                }

                let start_x = ui_px_from_gpui(bounds.origin.x + cursor_offset.x);
                drag_runtime.update(cx, |runtime, _| {
                    runtime.column_resize = TableColumnResizeState::begin(
                        drag.column_id.clone(),
                        start_x,
                        drag.start_width,
                        drag.column_widths_start.clone(),
                    );
                });
                window.prevent_default();
                cx.stop_propagation();
                cx.new(|_| Empty)
            },
        )
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            finish_table_column_resize(
                &mouse_up_runtime,
                &mouse_up_config,
                &drag_for_mouse_up,
                ui_px_from_gpui(event.position.x),
                window,
                cx,
            );
        })
        .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
            finish_table_column_resize(
                &mouse_up_out_runtime,
                &mouse_up_out_config,
                &drag_for_mouse_up_out,
                ui_px_from_gpui(event.position.x),
                window,
                cx,
            );
        })
        .child(
            div()
                .absolute()
                .right(px(0.0))
                .top(px(4.0))
                .bottom(px(4.0))
                .w(px(1.0))
                .bg(rgba(0xc8cdc2ff)),
        )
}

pub(super) fn render_table_column_order_drop_zone(
    table_id: String,
    target_column: TableColumnRenderPlan,
    placement: TableColumnOrderPlacement,
    handler: TableColumnOrderHandler,
    zone_width: Pixels,
    right_inset: Pixels,
) -> impl IntoElement {
    let target_column_id = target_column.id().clone();
    let target_region = target_column.region();
    let zone_key = target_column_id.as_str().to_owned();
    let placement_key = placement.as_str().to_owned();
    let table_for_can_drop = table_id.clone();
    let table_for_drag_over = table_id.clone();
    let table_for_drop = table_id.clone();
    let target_for_can_drop = target_column_id.clone();
    let target_for_drag_over = target_column_id.clone();
    let target_for_drop = target_column_id.clone();

    div()
        .id(format!(
            "table:{table_id}:header-order-drop:{placement_key}:{zone_key}"
        ))
        .debug_selector(move || {
            format!("table:{table_id}:header-order-drop:{placement_key}:{zone_key}")
        })
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .when(placement == TableColumnOrderPlacement::Before, |this| {
            this.left(px(0.0)).w(zone_width)
        })
        .when(placement == TableColumnOrderPlacement::After, |this| {
            this.right(right_inset).w(zone_width)
        })
        .can_drop(move |dragged, _, _| {
            dragged
                .downcast_ref::<TableColumnOrderDrag>()
                .is_some_and(|drag| {
                    drag.table_id == table_for_can_drop
                        && drag.region == target_region
                        && drag.column_id != target_for_can_drop
                })
        })
        .drag_over::<TableColumnOrderDrag>(move |style, drag, _, _| {
            if drag.table_id != table_for_drag_over
                || drag.region != target_region
                || drag.column_id == target_for_drag_over
            {
                return style;
            }

            style.bg(rgba(0x1f7a662e))
        })
        .on_drop(move |drag: &TableColumnOrderDrag, window, cx| {
            if drag.table_id != table_for_drop
                || drag.region != target_region
                || drag.column_id == target_for_drop
            {
                return;
            }

            let change = match placement {
                TableColumnOrderPlacement::Before => TableColumnOrderChange::move_before(
                    drag.column_id.clone(),
                    target_column_id.clone(),
                    drag.region,
                ),
                TableColumnOrderPlacement::After => TableColumnOrderChange::move_after(
                    drag.column_id.clone(),
                    target_column_id.clone(),
                    drag.region,
                ),
            };
            handler(change, window, cx);
        })
        .into_any_element()
}

pub(super) fn handle_table_column_resize_drag(
    runtime: &Entity<TableRuntime>,
    config: &TableResizeRenderConfig,
    event: &DragMoveEvent<TableColumnResizeDrag>,
    window: &mut Window,
    cx: &mut App,
) {
    let drag = event.drag(cx).clone();
    if drag.table_id != config.table_id {
        return;
    }

    let client_x = ui_px_from_gpui(event.event.position.x);
    let mut committed_change = None;
    runtime.update(cx, |runtime, _| {
        if runtime.column_resize.active_column().is_none() {
            runtime.column_resize = TableColumnResizeState::begin(
                drag.column_id.clone(),
                client_x,
                drag.start_width,
                drag.column_widths_start.clone(),
            );
        }

        let update = drag_table_column_resize(
            drag.mode,
            drag.direction,
            &drag.base_sizing,
            &runtime.column_resize,
            client_x,
        );
        if let Some(sizing) = update.committed_sizing().cloned() {
            committed_change = Some(table_column_sizing_change(&drag, sizing));
        }
        runtime.column_resize = update.state().clone();
    });

    if let (Some(handler), Some(change)) = (&config.on_change, committed_change) {
        handler(change, window, cx);
    }

    window.prevent_default();
    cx.stop_propagation();
    window.refresh();
}

fn finish_table_column_resize(
    runtime: &Entity<TableRuntime>,
    config: &TableResizeRenderConfig,
    drag: &TableColumnResizeDrag,
    client_x: UiPx,
    window: &mut Window,
    cx: &mut App,
) {
    if drag.table_id != config.table_id {
        return;
    }

    let mut committed_change = None;
    let mut handled = false;
    runtime.update(cx, |runtime, _| {
        if !runtime
            .column_resize
            .active_column()
            .is_some_and(|column_id| column_id == &drag.column_id)
        {
            return;
        }
        handled = true;

        let update = end_table_column_resize(
            drag.mode,
            drag.direction,
            &drag.base_sizing,
            &runtime.column_resize,
            Some(client_x),
        );
        if let Some(sizing) = update.committed_sizing().cloned() {
            committed_change = Some(table_column_sizing_change(drag, sizing));
        }
        runtime.column_resize = update.state().clone();
    });

    if !handled {
        return;
    }

    if let (Some(handler), Some(change)) = (&config.on_change, committed_change) {
        handler(change, window, cx);
    }

    window.prevent_default();
    cx.stop_propagation();
    window.refresh();
}

fn table_column_sizing_change(
    drag: &TableColumnResizeDrag,
    sizing: TableColumnSizing,
) -> TableColumnSizingChange {
    let width = sizing.width(&drag.column_id).unwrap_or(drag.start_width);
    TableColumnSizingChange::new(drag.column_id.clone(), width, sizing)
}
