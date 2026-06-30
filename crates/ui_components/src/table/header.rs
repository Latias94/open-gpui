use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    ClickEvent, CursorStyle, Empty, FontWeight, IntoElement, KeyDownEvent, ParentElement,
    ScrollHandle, Styled, div, px, rgb,
};
use open_gpui_ui_core::{
    Role, Sizable, TableColumnId, TableColumnRegion, TableSortDirection, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;

use super::resize::{
    TableColumnOrderDrag, TableResizeRenderConfig, render_table_column_order_drop_zone,
    render_table_resize_handle,
};
use super::{
    TableCenterColumnWindowPlan, TableColumnOrderHandler, TableColumnOrderPlacement,
    TableColumnRenderPlan, TableHeaderCellRenderPlan, TableMetrics, TableRenderPlan,
    TableSortHandler,
};

pub(super) fn render_table_header(
    plan: &TableRenderPlan,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
    resize_config: TableResizeRenderConfig,
    horizontal_scroll_handle: ScrollHandle,
    header_band_height: UiPx,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let column_header_role = plan.column_header_role();
    let regions = plan.column_regions().to_vec();
    let header_groups = plan.header_groups().clone();
    let columns_by_id = Rc::new(
        plan.columns()
            .iter()
            .cloned()
            .map(|column| (column.id().clone(), column))
            .collect::<BTreeMap<_, _>>(),
    );
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let rendered_center_leaf_ids = center_window.as_ref().map(|window| {
        window
            .rendered_columns()
            .iter()
            .map(|column| column.id().clone())
            .collect::<BTreeSet<_>>()
    });
    div()
        .id(format!("table:{table_id}:header-row"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:header-row")
        })
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .h(gpui_px_from_ui(header_band_height))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf3f4ef))
        .ui_role(plan.row_role())
        .aria_row_index(1)
        .children(regions.iter().map(move |region_plan| {
            let table_id = table_id.clone();
            let pinned_layout = pinned_layout.clone();
            let center_window = center_window.clone();
            let on_sort_requested = on_sort_requested.clone();
            let on_column_order_change = on_column_order_change.clone();
            let resize_config = resize_config.clone();
            let header_groups = header_groups.clone();
            let columns_by_id = columns_by_id.clone();
            let rendered_center_leaf_ids = rendered_center_leaf_ids.clone();
            let metrics = metrics;
            let column_header_role = column_header_role;
            let region = region_plan.region();
            let region_name = region.as_str().to_owned();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| region_plan.total_width());
            let header_region = header_groups.region(region);
            let reorder_enabled =
                on_column_order_change.is_some() && region_plan.columns().len() > 1;
            let mut occupied_leaf_ids = BTreeSet::new();
            let mut header_children = Vec::new();
            for group in header_region.groups() {
                for header in group.headers() {
                    let effective_leaf_ids = header
                        .leaf_column_ids()
                        .iter()
                        .filter(|leaf_id| {
                            if region == TableColumnRegion::Center {
                                rendered_center_leaf_ids
                                    .as_ref()
                                    .is_none_or(|rendered| rendered.contains(*leaf_id))
                            } else {
                                true
                            }
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if effective_leaf_ids.is_empty() {
                        continue;
                    }
                    if header.kind().is_placeholder()
                        && effective_leaf_ids
                            .iter()
                            .all(|leaf_id| occupied_leaf_ids.contains(leaf_id))
                    {
                        continue;
                    }

                    header_children.push(
                        render_table_header_group_cell(
                            table_id.clone(),
                            metrics,
                            column_header_role,
                            header.clone(),
                            effective_leaf_ids.clone(),
                            columns_by_id.clone(),
                            on_sort_requested.clone(),
                            on_column_order_change.clone(),
                            reorder_enabled,
                            resize_config.clone(),
                        )
                        .into_any_element(),
                    );
                    if header.kind().is_leaf() {
                        occupied_leaf_ids.extend(effective_leaf_ids);
                    }
                }
            }
            let center_scroll_id = pinned_layout.as_ref().and_then(|layout| {
                (region == TableColumnRegion::Center && !region_plan.columns().is_empty())
                    .then(|| layout.header_center_scroll_id())
            });

            let region_lane = div()
                .id(format!("table:{table_id}:header-region:{region_name}"))
                .debug_selector({
                    let table_id = table_id.clone();
                    let region_name = region_name.clone();
                    move || format!("table:{table_id}:header-region:{region_name}")
                })
                .relative()
                .h_full()
                .min_w(px(0.0))
                .w(gpui_px_from_ui(region_width))
                .flex_none()
                .overflow_hidden()
                .children(header_children)
                .into_any_element();

            if let Some(center_scroll_id) = center_scroll_id {
                div()
                    .h_full()
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
        }))
}

#[allow(clippy::too_many_arguments)]
fn render_table_header_group_cell(
    table_id: String,
    metrics: TableMetrics,
    column_header_role: Role,
    header: TableHeaderCellRenderPlan,
    effective_leaf_ids: Vec<TableColumnId>,
    columns_by_id: Rc<BTreeMap<TableColumnId, TableColumnRenderPlan>>,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
    reorder_enabled: bool,
    resize_config: TableResizeRenderConfig,
) -> impl IntoElement {
    let header_id = header.id().to_owned();
    let header_kind = header.kind();
    let header_label = header.label().to_owned();
    let is_leaf = header_kind.is_leaf();
    let interactive_sort = is_leaf
        .then(|| header.sort_action().cloned().zip(on_sort_requested))
        .flatten();
    let leaf_column = is_leaf
        .then(|| {
            effective_leaf_ids
                .first()
                .and_then(|column_id| columns_by_id.get(column_id))
                .cloned()
        })
        .flatten();
    let order_drag = reorder_enabled
        .then(|| {
            leaf_column.clone().map(|column| TableColumnOrderDrag {
                table_id: table_id.clone(),
                column_id: column.id().clone(),
                region: column.region(),
            })
        })
        .flatten();
    let order_drop_target = reorder_enabled.then(|| leaf_column.clone()).flatten();
    let order_drop_handler = reorder_enabled.then_some(on_column_order_change).flatten();
    let show_resize_handle = resize_config.enabled && header.resizable();
    let row_span = header.row_span().max(1) as f32;
    let width = effective_leaf_ids
        .iter()
        .fold(UiPx::ZERO, |total, column_id| {
            total
                + columns_by_id
                    .get(column_id)
                    .map(|column| column.width())
                    .unwrap_or(UiPx::ZERO)
        });
    let start = effective_leaf_ids
        .first()
        .and_then(|column_id| columns_by_id.get(column_id))
        .map(|column| column.start())
        .unwrap_or(UiPx::ZERO);
    let aria_column_index = effective_leaf_ids
        .first()
        .and_then(|column_id| columns_by_id.get(column_id))
        .map(|column| column.aria_column_index())
        .unwrap_or(1);
    let sort_suffix = header
        .sort_direction()
        .map(|direction| match direction {
            TableSortDirection::Ascending => " ↑",
            TableSortDirection::Descending => " ↓",
        })
        .unwrap_or("");

    div()
        .id(header_id.clone())
        .debug_selector(move || header_id.clone())
        .absolute()
        .top(gpui_px_from_ui(
            metrics.header_height() * header.depth() as f32,
        ))
        .left(gpui_px_from_ui(start))
        .w(gpui_px_from_ui(width))
        .min_w(gpui_px_from_ui(width))
        .max_w(gpui_px_from_ui(width))
        .flex_none()
        .h(gpui_px_from_ui(metrics.header_height() * row_span))
        .min_h(px(0.0))
        .flex()
        .items_center()
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xd6d8ce))
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0x3f4a57))
        .truncate()
        .whitespace_nowrap()
        .ui_role(column_header_role)
        .aria_label(header.label().to_owned())
        .aria_column_index(aria_column_index)
        .when_some(interactive_sort, |this, (action, handler)| {
            let key_action = action.clone();
            let key_handler = handler.clone();

            this.focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0xe9ece3)))
                .on_click(move |_event: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    handler(action.clone(), window, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    if !matches!(event.keystroke.key.as_str(), "space" | "enter") {
                        return;
                    }

                    cx.stop_propagation();
                    key_handler(key_action.clone(), window, cx);
                })
        })
        .child(format!("{}{}", header_label, sort_suffix))
        .when_some(order_drag, |this, drag| {
            this.cursor(CursorStyle::OpenHand)
                .on_drag(drag, |_, _, _, _, cx| cx.new(|_| Empty))
        })
        .when_some(order_drop_target, |this, column| {
            this.when_some(order_drop_handler.clone(), |this, handler| {
                let drop_handle_inset = if show_resize_handle {
                    px(10.0)
                } else {
                    px(0.0)
                };
                let drop_zone_width = px((width.as_f32() * 0.5).max(12.0));

                this.child(render_table_column_order_drop_zone(
                    table_id.clone(),
                    column.clone(),
                    TableColumnOrderPlacement::Before,
                    handler.clone(),
                    drop_zone_width,
                    drop_handle_inset,
                ))
                .child(render_table_column_order_drop_zone(
                    table_id.clone(),
                    column,
                    TableColumnOrderPlacement::After,
                    handler,
                    drop_zone_width,
                    drop_handle_inset,
                ))
            })
        })
        .when(show_resize_handle, |this| {
            this.when_some(leaf_column.clone(), |this, column| {
                this.child(render_table_resize_handle(
                    table_id.clone(),
                    column,
                    resize_config.clone(),
                ))
            })
        })
}
