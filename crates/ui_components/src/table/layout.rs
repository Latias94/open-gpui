use std::collections::BTreeMap;

use open_gpui_ui_core::{TableColumnRegion, UiPx};

use super::{TableColumnRegionRenderPlan, TableColumnRenderPlan};

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
    let mut region_starts = TableColumnRegion::ALL
        .into_iter()
        .map(|region| (region, UiPx::ZERO))
        .collect::<BTreeMap<_, _>>();

    columns
        .into_iter()
        .map(|column| {
            let region = column.region();
            let start = region_starts.get(&region).copied().unwrap_or(UiPx::ZERO);
            region_starts.insert(region, start + column.width());
            column.with_start(start)
        })
        .collect()
}
