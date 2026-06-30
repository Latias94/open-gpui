use std::collections::{BTreeMap, BTreeSet};

use open_gpui_ui_core::{
    TableResolvedRow, TableRowId, UiPx, VirtualizerItemKey, VirtualizerResolvedState,
    VirtualizerState,
};

use super::{TableRowMeasureMode, TableRowRenderPlan};

pub(super) fn measured_virtualizer_state(
    rows: &[TableResolvedRow],
    row_measure_mode: TableRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    fallback_row_height: UiPx,
    overscan: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    duplicate_row_ids: &BTreeSet<TableRowId>,
) -> VirtualizerResolvedState {
    let mut state = VirtualizerState::new(rows.len(), fallback_row_height)
        .with_viewport_extent(viewport_extent)
        .with_overscan(overscan)
        .with_scroll_offset(scroll_offset);

    let item_keys = rows
        .iter()
        .map(|row| VirtualizerItemKey::new(row_render_key(row, duplicate_row_ids)))
        .collect::<Vec<_>>();
    state = state.with_item_keys(item_keys);

    if row_measure_mode.measured() {
        return state.resolve_known_size_window(|index| {
            let row = &rows[index];
            let render_key = row_render_key(row, duplicate_row_ids);
            (
                VirtualizerItemKey::new(render_key.clone()),
                row_measurements
                    .get(&render_key)
                    .copied()
                    .unwrap_or(fallback_row_height),
            )
        });
    }

    state.resolve_fixed_window(|index| {
        let row = &rows[index];
        VirtualizerItemKey::new(row_render_key(row, duplicate_row_ids))
    })
}

pub(super) fn table_rows_virtual_size(rows: &[TableRowRenderPlan]) -> UiPx {
    rows.iter()
        .fold(UiPx::ZERO, |total, row| total + row.virtual_size())
}

pub(super) fn row_render_key(
    row: &TableResolvedRow,
    duplicate_row_ids: &BTreeSet<TableRowId>,
) -> String {
    if duplicate_row_ids.contains(row.id())
        && let Some(source_index) = row.source_index()
    {
        format!("{}:{}", source_index, row.id().as_str())
    } else {
        row.id().as_str().to_owned()
    }
}
