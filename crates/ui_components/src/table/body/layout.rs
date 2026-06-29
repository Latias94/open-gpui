use open_gpui::{AnyElement, IntoElement, Styled, div, px};
use open_gpui_ui_core::UiPx;

use crate::geometry::gpui_px_from_ui;
use crate::table::{TableCellRenderPlan, TableCenterColumnWindowPlan};

pub(in crate::table::body) fn table_row_region_cells_for_window(
    source_cells: &[TableCellRenderPlan],
    center_window: Option<&TableCenterColumnWindowPlan>,
) -> Vec<TableCellRenderPlan> {
    let Some(center_window) = center_window else {
        return source_cells.to_vec();
    };

    let cells_by_column = source_cells
        .iter()
        .map(|cell| (cell.column_id(), cell))
        .collect::<std::collections::BTreeMap<_, _>>();

    center_window
        .rendered_columns()
        .iter()
        .filter_map(|column| {
            cells_by_column
                .get(column.id())
                .map(|cell| (**cell).clone())
        })
        .collect()
}

pub(in crate::table::body) fn render_table_lane_spacer(width: UiPx) -> AnyElement {
    div()
        .w(gpui_px_from_ui(width))
        .min_w(px(0.0))
        .flex_none()
        .h_full()
        .min_h(px(0.0))
        .into_any_element()
}
