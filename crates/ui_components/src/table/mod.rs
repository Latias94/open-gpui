//! Table component backed by renderer-neutral row-model and virtualizer contracts.

mod body;
mod cell;
mod column_visibility;
mod content_fit;
mod editing;
mod editors;
mod faceted_filter;
mod filtering;
mod global_filter;
mod header;
mod interaction;
mod layout;
mod metrics;
mod predicate_filter;
mod range_filter;
mod render_plan;
mod resize;
mod resolve;
mod runtime;
mod toolbar;
mod virtualization;

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use open_gpui::prelude::*;
use open_gpui::{
    App, DragMoveEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, rgb,
};
use open_gpui_ui_core::{
    Sizable, Size, TableColumnResizeDirection, TableColumnResizeMode, TableExpansionMode,
    TableRowId, TableState, UiPx, VirtualizerSnapshot, ui_px,
};
pub use open_gpui_ui_core::{
    TableResolvedHeaderCell, TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions,
    TableResolvedHeaderKind,
};
use std::rc::Rc;

use body::{handle_table_vertical_scroll_wheel, render_table_body};
pub use column_visibility::{
    TableColumnVisibility, TableColumnVisibilityAction, TableColumnVisibilityChange,
    TableColumnVisibilityItemState, TableColumnVisibilityState,
};
use content_fit::apply_table_content_fit_widths;
pub use editing::{TableCellEditApplyOutcome, TableCellEditChange};
pub use faceted_filter::{
    TableFacetedFilter, TableFacetedFilterChange, TableFacetedFilterOptionState,
    TableFacetedFilterState,
};
pub use global_filter::{TableGlobalFilter, TableGlobalFilterChange, TableGlobalFilterState};
use header::render_table_header;
pub use interaction::{
    TableColumnOrderChange, TableColumnOrderPlacement, TableColumnSizingChange, TableHeaderAction,
    TableInputModifiers, TableRowAction, TableRowActivation, TableRowActivationKind,
    TableRowExpansionToggle, TableRowSelectionChange, TableSelectionScope,
};
use layout::resolve_table_column_offsets;
pub use metrics::TableMetrics;
pub use predicate_filter::{
    TablePredicateFilter, TablePredicateFilterChange, TablePredicateFilterOperator,
    TablePredicateFilterOperatorOptionState, TablePredicateFilterState,
};
pub use range_filter::{TableRangeFilter, TableRangeFilterChange, TableRangeFilterState};
pub use render_plan::{
    TableCellRenderPlan, TableCenterColumnWindowPlan, TableColumnRegionRenderPlan,
    TableColumnRenderPlan, TableHeaderCellRenderPlan, TableHeaderGroupRegionRenderPlan,
    TableHeaderGroupRegionsRenderPlan, TableHeaderGroupRenderPlan, TablePinnedLayoutPlan,
    TableRenderDiagnostics, TableRowRenderPlan,
};
use resize::{TableColumnResizeDrag, TableResizeRenderConfig, handle_table_column_resize_drag};
use runtime::TableRuntime;
pub use toolbar::{TableToolbar, TableToolbarState};

type TableSortHandler = Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>;
pub(super) type TableColumnSizingHandler =
    Rc<dyn Fn(TableColumnSizingChange, &mut Window, &mut App)>;
pub(super) type TableColumnOrderHandler = Rc<dyn Fn(TableColumnOrderChange, &mut Window, &mut App)>;
type TableRowActivationHandler = Rc<dyn Fn(TableRowActivation, &mut Window, &mut App)>;
type TableRowExpansionHandler = Rc<dyn Fn(TableRowExpansionToggle, &mut Window, &mut App)>;
type TableRowSelectionHandler = Rc<dyn Fn(TableRowSelectionChange, &mut Window, &mut App)>;
type TableCellEditHandler = Rc<dyn Fn(TableCellEditChange, &mut Window, &mut App)>;

/// Body row height ownership for table rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableRowMeasureMode {
    /// Body rows keep the shared fixed height contract.
    #[default]
    Fixed,
    /// Body rows may grow to fit their rendered content and feed measurements back into the virtualizer.
    Measured,
}

impl TableRowMeasureMode {
    /// Returns whether the table should measure row heights from rendered content.
    pub const fn measured(self) -> bool {
        matches!(self, Self::Measured)
    }
}

/// A concrete GPUI table renderer using the Open GPUI row-model and virtualizer contracts.
#[derive(IntoElement)]
pub struct Table {
    pub(super) id: String,
    pub(super) label: SharedString,
    pub(super) state: TableState,
    pub(super) metrics: TableMetrics,
    pub(super) row_measure_mode: TableRowMeasureMode,
    pub(super) snapshot: Option<VirtualizerSnapshot>,
    default_focused_row: Option<TableRowId>,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
    enable_column_resizing: bool,
    column_resize_mode: TableColumnResizeMode,
    column_resize_direction: TableColumnResizeDirection,
    on_column_sizing_change: Option<TableColumnSizingHandler>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
}

impl Table {
    /// Creates a new table renderer from renderer-neutral table state.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>, state: TableState) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state,
            metrics: TableMetrics::from_size(Size::Medium),
            row_measure_mode: TableRowMeasureMode::default(),
            snapshot: None,
            default_focused_row: None,
            on_sort_requested: None,
            on_column_order_change: None,
            enable_column_resizing: true,
            column_resize_mode: TableColumnResizeMode::default(),
            column_resize_direction: TableColumnResizeDirection::default(),
            on_column_sizing_change: None,
            on_row_selection_change: None,
            on_row_activate: None,
            on_row_expansion_request: None,
            on_cell_edit_change: None,
        }
    }

    /// Applies the accessible table label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Applies the body row height ownership mode.
    pub fn row_measure_mode(mut self, row_measure_mode: TableRowMeasureMode) -> Self {
        self.row_measure_mode = row_measure_mode;
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics.set_overscan(overscan);
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics.set_row_height(nonnegative_px(row_height));
        self
    }

    /// Applies a fixed header height.
    pub fn header_height(mut self, header_height: UiPx) -> Self {
        self.metrics
            .set_header_height(nonnegative_px(header_height));
        self
    }

    /// Applies the fallback viewport extent used before layout metrics exist.
    pub fn viewport_extent(mut self, viewport_extent: UiPx) -> Self {
        self.metrics
            .set_viewport_extent(nonnegative_px(viewport_extent));
        self
    }

    /// Applies the source-tree expansion mode.
    pub fn expansion_mode(mut self, expansion_mode: TableExpansionMode) -> Self {
        self.state = self.state.clone().with_expansion_mode(expansion_mode);
        self
    }

    /// Applies the minimum visual column width.
    pub fn min_column_width(mut self, min_column_width: UiPx) -> Self {
        self.metrics
            .set_min_column_width(nonnegative_px(min_column_width));
        self
    }

    /// Seeds table virtualizer measurements from a snapshot.
    ///
    /// The adapter applies the live `ScrollHandle` offset after restoring snapshot measurements.
    /// One-shot scroll-position restoration belongs to the runtime scroll owner.
    pub fn virtualizer_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Seeds the adapter-owned focused row.
    pub fn default_focused_row(mut self, row_id: impl Into<TableRowId>) -> Self {
        self.default_focused_row = Some(row_id.into());
        self
    }

    /// Registers a handler for sortable column header activation.
    pub fn on_sort_requested(
        mut self,
        handler: impl Fn(TableHeaderAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort_requested = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled column reorder changes.
    pub fn on_column_order_change(
        mut self,
        handler: impl Fn(TableColumnOrderChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_order_change = Some(Rc::new(handler));
        self
    }

    /// Enables or disables column resizing handles.
    pub fn enable_column_resizing(mut self, enabled: bool) -> Self {
        self.enable_column_resizing = enabled;
        self
    }

    /// Applies the resize commit mode.
    pub fn column_resize_mode(mut self, mode: TableColumnResizeMode) -> Self {
        self.column_resize_mode = mode;
        self
    }

    /// Applies the resize direction used for pointer deltas.
    pub fn column_resize_direction(mut self, direction: TableColumnResizeDirection) -> Self {
        self.column_resize_direction = direction;
        self
    }

    /// Registers a handler for committed column sizing changes.
    pub fn on_column_sizing_change(
        mut self,
        handler: impl Fn(TableColumnSizingChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_sizing_change = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled row selection changes.
    pub fn on_row_selection_change(
        mut self,
        handler: impl Fn(TableRowSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_selection_change = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for row activation gestures.
    pub fn on_row_activate(
        mut self,
        handler: impl Fn(TableRowActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled row expansion requests.
    pub fn on_row_expansion_request(
        mut self,
        handler: impl Fn(TableRowExpansionToggle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_expansion_request = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled text-cell edit changes.
    pub fn on_cell_edit_change(
        mut self,
        handler: impl Fn(TableCellEditChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cell_edit_change = Some(Rc::new(handler));
        self
    }

    /// Returns the renderer-neutral table input.
    pub const fn state(&self) -> &TableState {
        &self.state
    }
}

impl Sizable for Table {
    fn with_size(mut self, size: Size) -> Self {
        self.metrics = TableMetrics::from_size(size);
        self
    }
}

impl RenderOnce for Table {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("table:{}:runtime", self.id);
        let default_focused_row = self.default_focused_row.clone();
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| {
            TableRuntime::new(default_focused_row)
        });
        let scroll_handle = runtime.read(cx).scroll_handle.clone();
        let horizontal_scroll_handle = runtime.read(cx).horizontal_scroll_handle.clone();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let scroll_offset = ui_px((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let on_sort_requested = self.on_sort_requested.clone();
        let on_column_order_change = self.on_column_order_change.clone();
        let column_resizing_enabled =
            self.enable_column_resizing && self.on_column_sizing_change.is_some();
        let resize_config = TableResizeRenderConfig {
            table_id: self.id.clone(),
            enabled: column_resizing_enabled,
            mode: self.column_resize_mode,
            direction: self.column_resize_direction,
            base_sizing: self.state.column_sizing().clone(),
            runtime: runtime.clone(),
            on_change: self.on_column_sizing_change.clone(),
        };
        let resize_drag_runtime = resize_config.runtime.clone();
        let resize_drag_config = resize_config.clone();
        let plan = runtime.update(cx, |runtime, cx| {
            let plan = self.diagnostics_with_runtime(
                scroll_offset,
                viewport_extent,
                horizontal_scroll_handle.clone(),
                window,
                runtime,
            );
            runtime.sync_rows(&plan, cx);
            plan
        });
        let runtime_snapshot = runtime.read(cx).clone();
        let current_expansion = runtime_snapshot
            .expansion_override
            .clone()
            .unwrap_or_else(|| self.state.expansion().clone());
        let table_id = plan.table_id().to_owned();
        let label = plan.label().to_owned();
        let metrics = plan.metrics();
        let scroll_viewport_id = format!("table:{table_id}:body-scroll");
        let selection_policy = plan.selection_policy();
        let selected_row_ids = Rc::new(
            plan.table()
                .core_model()
                .rows()
                .iter()
                .filter(|row| row.selected())
                .map(|row| row.id().clone())
                .collect::<Vec<_>>(),
        );
        let on_row_activate = self.on_row_activate.clone();
        let on_row_selection_change = self.on_row_selection_change.clone();
        let on_row_expansion_request = self.on_row_expansion_request.clone();
        let on_cell_edit_change = self.on_cell_edit_change.clone();

        div()
            .id(self.id)
            .debug_selector({
                let table_id = table_id.clone();
                move || format!("table:{table_id}:root")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(gpui_px_from_ui(metrics.size().control_text_px()))
            .text_color(rgb(0x2f3845))
            .ui_role(plan.role())
            .aria_label(label)
            .when(plan.aria_row_count() > 0, |this| {
                this.aria_row_count(plan.aria_row_count())
            })
            .when(plan.aria_column_count() > 0, |this| {
                this.aria_column_count(plan.aria_column_count())
            })
            .on_scroll_wheel({
                let scroll_handle = scroll_handle.clone();
                move |event, window, cx| {
                    handle_table_vertical_scroll_wheel(&scroll_handle, event, window);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .when(resize_config.enabled, |this| {
                this.on_drag_move(
                    move |event: &DragMoveEvent<TableColumnResizeDrag>, window, cx| {
                        handle_table_column_resize_drag(
                            &resize_drag_runtime,
                            &resize_drag_config,
                            event,
                            window,
                            cx,
                        );
                    },
                )
            })
            .child(render_table_body(
                &plan,
                scroll_viewport_id,
                horizontal_scroll_handle.clone(),
                scroll_handle.clone(),
                plan.sticky_header_band_height(),
                runtime.clone(),
                runtime_snapshot,
                current_expansion,
                selection_policy,
                selected_row_ids,
                on_row_selection_change,
                on_row_activate,
                on_row_expansion_request,
                on_cell_edit_change,
            ))
            .child(render_table_header(
                &plan,
                on_sort_requested,
                on_column_order_change,
                resize_config,
                horizontal_scroll_handle.clone(),
                plan.sticky_header_band_height(),
            ))
    }
}

pub(super) const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::virtualization::measured_virtualizer_state;
    use super::*;
    use open_gpui_ui_core::{
        TableColumn, TableColumnId, TableColumnPinning, TableColumnRegion, TableColumnSizing,
        TableRow, TableSort, VirtualizerRange,
    };

    #[test]
    fn apply_table_content_fit_widths_keeps_committed_widths_authoritative() {
        let committed_width = ui_px(72.0);
        let column = TableColumnRenderPlan::test_content_fit_column(
            "status",
            "Status",
            committed_width,
            ui_px(10.0),
            ui_px(240.0),
        );

        let measured_widths = BTreeMap::from([(TableColumnId::new("status"), ui_px(128.0))]);
        let committed_sizing = TableColumnSizing::new().with_width("status", committed_width);
        let columns =
            apply_table_content_fit_widths(vec![column], &measured_widths, &committed_sizing);

        assert_eq!(columns[0].width(), committed_width);
        assert_eq!(columns[0].start(), UiPx::ZERO);
        assert_eq!(columns[0].after(), UiPx::ZERO);
    }

    #[test]
    fn table_column_order_change_reorders_leaf_columns_without_touching_other_state() {
        let state = TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 42_usize)])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ])
        .with_column_order(["name", "team", "score"])
        .with_sorting([TableSort::descending("score")])
        .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]));

        let change =
            TableColumnOrderChange::move_before("score", "team", TableColumnRegion::Center);
        let next = change.apply_to(state.clone());

        assert_eq!(
            next.column_order()
                .iter()
                .map(|column_id| column_id.as_str())
                .collect::<Vec<_>>(),
            ["name", "score", "team"]
        );
        assert_eq!(next.sorting(), state.sorting());
        assert_eq!(next.column_pinning(), state.column_pinning());
        assert_eq!(
            change.apply_to_order(state.column_order().iter().cloned()),
            vec![
                TableColumnId::new("name"),
                TableColumnId::new("score"),
                TableColumnId::new("team"),
            ]
        );
    }

    #[test]
    fn measured_virtualizer_uses_cached_row_heights_for_known_rows() {
        let resolved = TableState::new([
            TableRow::new("row-a").with_cell("name", "Alpha"),
            TableRow::new("row-b").with_cell("name", "Beta"),
            TableRow::new("row-c").with_cell("name", "Gamma"),
        ])
        .with_columns([TableColumn::new("name", "Name")])
        .resolve();
        let rows = resolved.center_rows();
        let duplicate_row_ids = resolved
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let measurements = BTreeMap::from([
            (rows[0].id().as_str().to_owned(), ui_px(18.0)),
            (rows[1].id().as_str().to_owned(), ui_px(28.0)),
        ]);

        let resolved = measured_virtualizer_state(
            &rows,
            TableRowMeasureMode::Measured,
            &measurements,
            ui_px(20.0),
            2,
            ui_px(0.0),
            ui_px(60.0),
            &duplicate_row_ids,
        );

        assert_eq!(resolved.total_size(), ui_px(66.0));
        assert_eq!(*resolved.visible_range(), VirtualizerRange::new(0, 3));
        assert_eq!(resolved.items().len(), 3);
        assert_eq!(resolved.measurements()[0].size(), ui_px(18.0));
        assert_eq!(resolved.measurements()[1].size(), ui_px(28.0));
        assert_eq!(resolved.measurements()[2].size(), ui_px(20.0));
        assert_eq!(resolved.measurements()[0].start(), ui_px(0.0));
        assert_eq!(resolved.measurements()[1].start(), ui_px(18.0));
        assert_eq!(resolved.measurements()[2].start(), ui_px(46.0));
    }
}
