use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui::{ScrollHandle, Window};
use open_gpui_ui_core::{
    TableColumnRegion, TableExpansionState, TableResolvedState, TableState, UiPx,
    VirtualizerItemKey, VirtualizerResolvedState, VirtualizerState, ui_px,
};

use super::content_fit::{content_fit_measure_key, table_content_fit_rendered_rows};
use super::runtime::{TableResolvedCache, TableRuntime};
use super::virtualization::{measured_virtualizer_state, row_render_key};
use super::{
    Table, TableBehaviorSnapshot, TableColumnRenderPlan, TableMetrics, TableRenderDiagnostics,
    nonnegative_px,
};
use crate::geometry::ui_px_from_gpui;

impl Table {
    /// Resolves table row models and virtual render windows for internal rendering.
    pub(in crate::table) fn diagnostics(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> TableRenderDiagnostics {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let table = Rc::new(self.state.resolve());
        let columns = self.resolve_columns(&table);
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let virtualizer = if self.row_measure_mode.measured() {
            measured_virtualizer_state(
                table.center_rows(),
                self.row_measure_mode,
                &BTreeMap::new(),
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
                &duplicate_row_ids,
            )
        } else {
            self.resolve_virtualizer(&table, metrics, scroll_offset)
        };

        TableRenderDiagnostics::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            self.row_measure_mode,
            &self.state,
            table,
            virtualizer,
            columns,
            BTreeMap::new(),
            None,
            None,
            &BTreeMap::new(),
        )
    }

    /// Resolves a public, user-observable behavior snapshot for a viewport.
    pub fn behavior_snapshot(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> TableBehaviorSnapshot {
        let plan = self.diagnostics(scroll_offset, viewport_extent);
        TableBehaviorSnapshot::from_diagnostics(&plan, &self.state)
    }

    pub(super) fn diagnostics_with_runtime(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        horizontal_scroll_handle: ScrollHandle,
        window: &Window,
        runtime: &mut TableRuntime,
    ) -> TableRenderDiagnostics {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let state = runtime
            .expansion_override
            .as_ref()
            .cloned()
            .map(|expansion| apply_table_expansion(self.state.clone(), expansion))
            .unwrap_or_else(|| self.state.clone());
        let cache_key = state.cache_key();
        let needs_resolve = runtime
            .resolved
            .as_ref()
            .map(|cache| cache.key != cache_key)
            .unwrap_or(true);
        if needs_resolve {
            let table = Rc::new(state.resolve());
            let columns = self.resolve_columns(&table);
            runtime.clear_row_measurements();
            runtime.resolved = Some(TableResolvedCache {
                key: cache_key,
                table,
                columns,
            });
        }

        let cache = runtime
            .resolved
            .as_ref()
            .expect("table runtime cache should be initialized");
        let virtualizer = if self.row_measure_mode.measured() {
            measured_virtualizer_state(
                cache.table.center_rows(),
                self.row_measure_mode,
                &runtime.row_measurements,
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
                &cache.table.duplicate_row_ids().iter().cloned().collect(),
            )
        } else {
            self.resolve_virtualizer(&cache.table, metrics, scroll_offset)
        };
        let rendered_rows = table_content_fit_rendered_rows(&cache.table, &virtualizer);
        let center_scroll_offset =
            ui_px((-ui_px_from_gpui(horizontal_scroll_handle.offset().x).as_f32()).max(0.0));
        let center_viewport_extent = ui_px_from_gpui(horizontal_scroll_handle.bounds().size.width);
        let center_viewport_extent =
            (center_viewport_extent.as_f32() > 0.0).then_some(center_viewport_extent);
        let center_scroll_offset = center_viewport_extent.map(|_| center_scroll_offset);
        let content_fit_widths = runtime
            .content_fit
            .widths_for(
                content_fit_measure_key(
                    cache.key.clone(),
                    metrics,
                    &cache.columns,
                    &rendered_rows,
                    window,
                ),
                &cache.columns,
                &rendered_rows,
                metrics,
                window,
            )
            .clone();

        TableRenderDiagnostics::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            self.row_measure_mode,
            &state,
            cache.table.clone(),
            virtualizer,
            cache.columns.clone(),
            content_fit_widths,
            center_scroll_offset,
            center_viewport_extent,
            &runtime.row_measurements,
        )
    }

    fn metrics_for_viewport(&self, viewport_extent: UiPx) -> TableMetrics {
        let mut metrics = self.metrics;
        let viewport_extent = nonnegative_px(viewport_extent);
        if viewport_extent.as_f32() > 0.0 {
            metrics.set_viewport_extent(viewport_extent);
        }
        metrics
    }

    fn resolve_virtualizer(
        &self,
        table: &TableResolvedState,
        metrics: TableMetrics,
        scroll_offset: UiPx,
    ) -> VirtualizerResolvedState {
        let center_rows = table.center_rows();
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let virtualizer = VirtualizerState::new(center_rows.len(), metrics.row_height())
            .with_viewport_extent(metrics.viewport_extent())
            .with_overscan(metrics.overscan())
            .with_scroll_offset(nonnegative_px(scroll_offset));

        if let Some(snapshot) = self.snapshot.clone() {
            let row_keys = center_rows
                .iter()
                .map(|row| row_render_key(row, &duplicate_row_ids));
            return virtualizer
                .with_item_keys(row_keys)
                .with_snapshot(snapshot)
                .with_scroll_offset(nonnegative_px(scroll_offset))
                .resolve();
        }

        virtualizer.resolve_fixed_window(|index| {
            let row = &center_rows[index];
            VirtualizerItemKey::new(row_render_key(row, &duplicate_row_ids))
        })
    }

    fn resolve_columns(&self, table: &TableResolvedState) -> Vec<TableColumnRenderPlan> {
        let mut aria_column_index = 1;
        let mut columns = Vec::new();
        let visible_regions = table.visible_column_regions();
        let visible_sizing = table.visible_column_sizing();

        for region in TableColumnRegion::ALL {
            for column in visible_regions.region(region) {
                let sizing = visible_sizing
                    .column(column.id())
                    .expect("visible column sizing should resolve for visible columns");
                let sort_direction = self
                    .state
                    .sorting()
                    .iter()
                    .find(|sort| sort.column() == column.id())
                    .map(|sort| sort.direction());
                columns.push(TableColumnRenderPlan::new(
                    column,
                    sizing,
                    region,
                    aria_column_index,
                    sort_direction,
                ));
                aria_column_index += 1;
            }
        }

        columns
    }
}

pub(super) fn apply_table_expansion(
    state: TableState,
    expansion: TableExpansionState,
) -> TableState {
    match expansion {
        TableExpansionState::All => state.with_all_rows_expanded(),
        TableExpansionState::Rows(rows) => state.with_expanded_rows(rows),
    }
}
