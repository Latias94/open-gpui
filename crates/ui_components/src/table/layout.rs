use std::collections::BTreeMap;

use open_gpui_ui_core::{TableColumnRegion, UiPx};

use super::{TableColumnRegionRenderPlan, TableColumnRenderPlan, nonnegative_px};

pub(super) fn resolve_column_region_render_plans(
    columns: &[TableColumnRenderPlan],
) -> Vec<TableColumnRegionRenderPlan> {
    TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            TableColumnRegionRenderPlan::new(
                region,
                columns
                    .iter()
                    .filter(|column| column.region() == region)
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn resolve_table_column_offsets(
    columns: Vec<TableColumnRenderPlan>,
) -> Vec<TableColumnRenderPlan> {
    let region_totals = TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            let total = columns
                .iter()
                .filter(|column| column.region() == region)
                .fold(UiPx::ZERO, |total, column| total + column.width());
            (region, total)
        })
        .collect::<BTreeMap<_, _>>();
    let mut region_starts = TableColumnRegion::ALL
        .into_iter()
        .map(|region| (region, UiPx::ZERO))
        .collect::<BTreeMap<_, _>>();

    columns
        .into_iter()
        .map(|column| {
            let region = column.region();
            let start = region_starts.get(&region).copied().unwrap_or(UiPx::ZERO);
            let total_width = region_totals.get(&region).copied().unwrap_or(UiPx::ZERO);
            let after = nonnegative_px(total_width - start - column.width());
            region_starts.insert(region, start + column.width());
            column.with_offsets(start, after)
        })
        .collect()
}
