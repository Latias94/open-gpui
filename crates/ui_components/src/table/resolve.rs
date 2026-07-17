use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{ScrollHandle, Window};
use open_gpui_ui_core::{
    TableColumnRegion, TableResolvedState, UiPx, VirtualizerResolvedState, VirtualizerState, ui_px,
};

use super::content_fit::{content_fit_measure_key, table_content_fit_rendered_rows};
use super::identity::table_row_virtualizer_key_from_key;
use super::runtime::{TableResolvedCache, TableRuntime};
use super::virtualization::measured_virtualizer_state;
use super::{
    Table, TableBehaviorSnapshot, TableColumnRenderPlan, TableMetrics, TableRenderPlan,
    TableVirtualizerSnapshot, nonnegative_px,
};
use crate::geometry::ui_px_from_gpui;

impl Table {
    /// Resolves table row models and virtual render windows for internal rendering.
    pub(in crate::table) fn render_plan(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> TableRenderPlan {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let table = Rc::new(self.state.resolve());
        let columns = self.resolve_columns(&table);
        let snapshot_measurements = if self.row_measure_mode.measured() {
            self.snapshot
                .as_ref()
                .map(TableVirtualizerSnapshot::effective_measurement_map)
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        let virtualizer = if self.row_measure_mode.measured() {
            measured_virtualizer_state(
                table.center_rows(),
                &snapshot_measurements,
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
            )
        } else {
            Self::resolve_fixed_virtualizer(&table, metrics, scroll_offset)
        };

        TableRenderPlan::resolve(
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
            &snapshot_measurements,
        )
    }

    /// Resolves a public, user-observable behavior snapshot for a viewport.
    pub fn behavior_snapshot(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> TableBehaviorSnapshot {
        let plan = self.render_plan(scroll_offset, viewport_extent);
        TableBehaviorSnapshot::from_render_plan(&plan, &self.state)
    }

    pub(super) fn render_plan_with_runtime(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        horizontal_scroll_handle: ScrollHandle,
        window: &Window,
        runtime: &mut TableRuntime,
    ) -> TableRenderPlan {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let state = self.state.clone();
        let cache_key = state.cache_key();
        let needs_resolve = runtime
            .resolved
            .as_ref()
            .map(|cache| cache.key != cache_key)
            .unwrap_or(true);
        if needs_resolve {
            runtime.advance_resolved_model_revision();
            let table = Rc::new(state.resolve());
            let columns = self.resolve_columns(&table);
            runtime.reconcile_row_measurements(&table);
            runtime.resolved = Some(TableResolvedCache {
                key: cache_key,
                table,
                columns,
            });
        }

        if self.row_measure_mode.measured()
            && runtime.apply_virtualizer_snapshot(self.snapshot.as_ref())
        {
            let table = runtime
                .resolved
                .as_ref()
                .expect("table runtime cache should be initialized")
                .table
                .clone();
            runtime.reconcile_row_measurements(&table);
        }

        let virtualizer = if self.row_measure_mode.measured() {
            let table = runtime
                .resolved
                .as_ref()
                .expect("table runtime cache should be initialized")
                .table
                .clone();
            runtime.resolve_measured_virtualizer(
                table.center_rows(),
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
            )
        } else {
            let cache = runtime
                .resolved
                .as_ref()
                .expect("table runtime cache should be initialized");
            Self::resolve_fixed_virtualizer(&cache.table, metrics, scroll_offset)
        };
        let cache = runtime
            .resolved
            .as_ref()
            .expect("table runtime cache should be initialized");
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

        TableRenderPlan::resolve(
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
            runtime,
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

    fn resolve_fixed_virtualizer(
        table: &TableResolvedState,
        metrics: TableMetrics,
        scroll_offset: UiPx,
    ) -> VirtualizerResolvedState {
        let center_rows = table.center_rows();
        let virtualizer = VirtualizerState::new(center_rows.len(), metrics.row_height())
            .with_viewport_extent(metrics.viewport_extent())
            .with_overscan(metrics.overscan())
            .with_scroll_offset(nonnegative_px(scroll_offset));

        virtualizer.resolve_fixed_window(|index| {
            let row = &center_rows[index];
            table_row_virtualizer_key_from_key(row.identity_key())
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
