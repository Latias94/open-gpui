//! Table component backed by renderer-neutral row-model and virtualizer contracts.

mod column_visibility;
mod content_fit;
mod editing;
mod faceted_filter;
mod filtering;
mod global_filter;
mod interaction;
mod layout;
mod metrics;
mod predicate_filter;
mod range_filter;
mod render_plan;
mod resolve;
mod runtime;
mod toolbar;
mod virtualization;

use crate::a11y::UiA11yElementExt;
use crate::checkbox::Checkbox;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::listbox::ListboxOption;
use crate::scroll_area::ScrollArea;
use crate::select::Select;
use crate::text_input::TextInput;
use crate::textarea::Textarea;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, CursorStyle, DragMoveEvent, Empty, Entity, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels,
    RenderOnce, ScrollHandle, ScrollWheelEvent, SharedString, StatefulInteractiveElement, Styled,
    Window, div, point, px, rgb, rgba,
};
use open_gpui_ui_core::{
    Role, Sizable, Size, TableCellEditor, TableCellValue, TableColumnId, TableColumnRegion,
    TableColumnResizeDirection, TableColumnResizeMode, TableColumnResizeState, TableColumnSizing,
    TableExpansionMode, TableExpansionState, TableResolvedRow, TableRowChildrenLoadState,
    TableRowId, TableRowRegion, TableSelectionPolicy, TableSortDirection, TableState, TableTreeRow,
    Toggled, UiPx, VirtualizerSnapshot, drag_table_column_resize, end_table_column_resize, ui_px,
};
pub use open_gpui_ui_core::{
    TableResolvedHeaderCell, TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions,
    TableResolvedHeaderKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

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
use interaction::request_table_row_selection_change;
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
    TableRenderPlan, TableRowRenderPlan,
};
use runtime::TableRuntime;
pub use toolbar::{TableToolbar, TableToolbarState};
use virtualization::table_rows_virtual_size;

type TableSortHandler = Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>;
type TableColumnSizingHandler = Rc<dyn Fn(TableColumnSizingChange, &mut Window, &mut App)>;
type TableColumnOrderHandler = Rc<dyn Fn(TableColumnOrderChange, &mut Window, &mut App)>;
type TableRowActivationHandler = Rc<dyn Fn(TableRowActivation, &mut Window, &mut App)>;
type TableRowExpansionHandler = Rc<dyn Fn(TableRowExpansionToggle, &mut Window, &mut App)>;
type TableRowSelectionHandler = Rc<dyn Fn(TableRowSelectionChange, &mut Window, &mut App)>;
type TableCellEditHandler = Rc<dyn Fn(TableCellEditChange, &mut Window, &mut App)>;

#[derive(Clone)]
struct TableResizeRenderConfig {
    table_id: String,
    enabled: bool,
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
    base_sizing: TableColumnSizing,
    runtime: Entity<TableRuntime>,
    on_change: Option<TableColumnSizingHandler>,
}

#[derive(Debug, Clone, PartialEq)]
struct TableColumnResizeDrag {
    table_id: String,
    column_id: TableColumnId,
    start_width: UiPx,
    column_widths_start: Vec<(TableColumnId, UiPx)>,
    base_sizing: TableColumnSizing,
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TableColumnOrderDrag {
    table_id: String,
    column_id: TableColumnId,
    region: TableColumnRegion,
}

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
    pub const fn table_state(&self) -> &TableState {
        &self.state
    }

    /// Returns a default resolved plan at scroll origin.
    pub fn state(&self) -> TableRenderPlan {
        self.render_plan(UiPx::ZERO, self.metrics.viewport_extent())
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
            let plan = self.render_plan_with_runtime(
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

fn render_table_header(
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
) -> AnyElement {
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
                .map(|column| column.clone())
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
        .into_any_element()
}

fn render_table_resize_handle(
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
                .bg(rgb(0xc8cdc2)),
        )
}

fn render_table_column_order_drop_zone(
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

fn handle_table_column_resize_drag(
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

fn toggle_table_expansion(
    expansion: TableExpansionState,
    row_id: TableRowId,
    expanded: bool,
) -> TableExpansionState {
    match expansion {
        TableExpansionState::All if expanded => TableExpansionState::All,
        TableExpansionState::All => TableExpansionState::default(),
        TableExpansionState::Rows(mut rows) => {
            if expanded {
                rows.insert(row_id);
            } else {
                rows.remove(&row_id);
            }
            TableExpansionState::Rows(rows)
        }
    }
}

fn render_table_body(
    plan: &TableRenderPlan,
    scroll_viewport_id: String,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    header_band_height: UiPx,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let final_rows = Rc::new(plan.table().final_model().rows().to_vec());
    let top_rows = plan.top_rows().to_vec();
    let center_rows = plan.rows().to_vec();
    let bottom_rows = plan.bottom_rows().to_vec();
    let top_row_count = top_rows.len();
    let center_total_row_count = plan.virtualizer().count();
    let top_height = table_rows_virtual_size(&top_rows);
    let center_height = plan.virtualizer().total_size();
    let bottom_height = table_rows_virtual_size(&bottom_rows);
    let measured_rows = plan.row_measure_mode().measured();

    div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .pt(gpui_px_from_ui(header_band_height))
        .flex()
        .flex_col()
        .when(!top_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Top,
                metrics,
                top_rows.clone(),
                top_height,
                pinned_layout.clone(),
                center_window.clone(),
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                runtime_snapshot.clone(),
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
                measured_rows,
            ))
        })
        .child(
            div().flex_1().min_h(px(0.0)).overflow_hidden().child(
                ScrollArea::new(
                    scroll_viewport_id,
                    render_table_row_band(
                        &table_id,
                        TableRowRegion::Center,
                        metrics,
                        center_rows,
                        center_height,
                        pinned_layout.clone(),
                        center_window.clone(),
                        horizontal_scroll_handle.clone(),
                        vertical_scroll_handle.clone(),
                        runtime.clone(),
                        runtime_snapshot.clone(),
                        final_rows.clone(),
                        top_row_count,
                        center_total_row_count,
                        current_expansion.clone(),
                        selection_policy,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        on_row_activate.clone(),
                        on_row_expansion_request.clone(),
                        on_cell_edit_change.clone(),
                        measured_rows,
                    ),
                )
                .vertical()
                .scroll_handle(&vertical_scroll_handle)
                .with_size(metrics.size()),
            ),
        )
        .when(!bottom_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Bottom,
                metrics,
                bottom_rows.clone(),
                bottom_height,
                pinned_layout,
                center_window,
                horizontal_scroll_handle,
                vertical_scroll_handle,
                runtime,
                runtime_snapshot,
                final_rows,
                top_row_count,
                center_total_row_count,
                current_expansion,
                selection_policy,
                selected_row_ids,
                on_row_selection_change,
                on_row_activate,
                on_row_expansion_request,
                on_cell_edit_change,
                measured_rows,
            ))
        })
}

fn render_table_row_band(
    table_id: &str,
    region: TableRowRegion,
    metrics: TableMetrics,
    rows: Vec<TableRowRenderPlan>,
    height: UiPx,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> AnyElement {
    let table_id = table_id.to_owned();
    let region_name = region.as_str();
    div()
        .id(format!("table:{table_id}:body:{region_name}"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:body:{region_name}")
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(height))
        .flex_none()
        .children(rows.into_iter().map(move |row| {
            let table_id = table_id.clone();
            let center_window = center_window.clone();
            let focus_handle = runtime_snapshot.focus_handles.get(row.id()).cloned();
            let focused = runtime_snapshot.focused_row.as_ref() == Some(row.id());
            render_table_row(
                table_id,
                row,
                metrics,
                pinned_layout.clone(),
                center_window,
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                focus_handle,
                focused,
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
                measured_rows,
            )
        }))
        .into_any_element()
}

fn render_table_row(
    table_id: String,
    row: TableRowRenderPlan,
    metrics: TableMetrics,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    focused: bool,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_id = row.id().clone();
    let row_for_layout = row.clone();
    let row_for_click = row.clone();
    let row_for_key = row.clone();
    let tree = row.row().tree().cloned();
    let tree_depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
    let tree_branch = row.row().is_tree_branch();
    let tree_expanded = row.row().tree_expanded().unwrap_or(false);
    let row_background = if row.row().is_group() {
        rgb(0xf1f4f8)
    } else if row.selected() {
        rgb(0xe7f0ff)
    } else if row.model_index().is_multiple_of(2) {
        rgb(0xffffff)
    } else {
        rgb(0xf8f9f3)
    };
    let region_cells = TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            let source_cells = row.cells_for_region(region).cloned().collect::<Vec<_>>();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let cells = table_row_region_cells_for_window(&source_cells, active_center_window);
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| {
                    source_cells
                        .iter()
                        .fold(UiPx::ZERO, |total, cell| total + cell.width())
                });
            let leading_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::leading_spacer_width)
                .unwrap_or(UiPx::ZERO);
            let trailing_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::trailing_spacer_width)
                .unwrap_or(UiPx::ZERO);
            (
                region,
                region_width,
                cells,
                !source_cells.is_empty(),
                leading_spacer_width,
                trailing_spacer_width,
                active_center_window.is_some(),
            )
        })
        .collect::<Vec<_>>();
    let tree_affordance_column_id = tree.as_ref().and_then(|_| {
        region_cells.iter().find_map(|(_, _, cells, _, _, _, _)| {
            cells.first().map(|cell| cell.column_id().clone())
        })
    });

    let row_element = div()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let row_key = render_key.clone();
            move |row_bounds, _window, cx| {
                if measured_rows {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max);
                    let measured_height = measured_height.ceil();
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            row_key.clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
        .id(format!("table:{table_id}:row:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let render_key = render_key.clone();
            move || format!("table:{table_id}:row:{render_key}")
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .min_w(px(0.0))
        .flex()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .hover(|this| this.bg(rgb(0xeef2f7)))
        .ui_role(row.role())
        .aria_row_index(row.aria_row_index())
        .aria_selected(row.selected())
        .when(tree_branch, |this| this.aria_expanded(tree_expanded))
        .focusable()
        .tab_stop(focused)
        .when_some(focus_handle.clone(), |this, focus_handle| {
            this.track_focus(&focus_handle)
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(!tree_branch || on_row_activate.is_some(), |this| {
            this.cursor_pointer()
        })
        .on_click({
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let selection_policy = selection_policy;
            let selected_row_ids = selected_row_ids.clone();
            let on_row_selection_change = on_row_selection_change.clone();
            let on_row_activate = on_row_activate.clone();
            move |event: &ClickEvent, window, cx| {
                if !event.standard_click() || window.default_prevented() {
                    return;
                }

                cx.stop_propagation();
                window.prevent_default();

                let action = TableRowAction::from_render_plan(
                    &row_for_click,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                if selection_policy.activation_mode().is_row_click() {
                    request_table_row_selection_change(
                        &runtime,
                        &action,
                        selection_policy,
                        TableSelectionScope::Row,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        window,
                        cx,
                    );
                }

                let activation_kind = if event.click_count() >= 2 {
                    TableRowActivationKind::DoubleClick
                } else {
                    TableRowActivationKind::Click
                };
                runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(row_id.clone(), cx);
                });
                if let Some(focus_handle) = focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                if let Some(on_row_activate) = on_row_activate.as_ref() {
                    on_row_activate(TableRowActivation::new(action, activation_kind), window, cx);
                }
            }
        })
        .on_key_down({
            let runtime = runtime.clone();
            let on_row_activate = on_row_activate.clone();
            let on_row_expansion_request = on_row_expansion_request.clone();
            let current_expansion_for_key = current_expansion.clone();
            move |event: &KeyDownEvent, window, cx| {
                handle_table_row_key_down(
                    &row_for_key,
                    final_rows.as_ref(),
                    vertical_scroll_handle.clone(),
                    top_row_count,
                    center_total_row_count,
                    &runtime,
                    current_expansion_for_key.clone(),
                    on_row_activate.clone(),
                    on_row_expansion_request.clone(),
                    event,
                    window,
                    cx,
                );
            }
        })
        .children(region_cells.into_iter().map(
            move |(
                region,
                region_width,
                cells,
                has_source_cells,
                leading_spacer_width,
                trailing_spacer_width,
                uses_center_window,
            )| {
                let table_id = table_id.clone();
                let render_key = render_key.clone();
                let region_name = region.as_str().to_owned();
                let center_scroll_id = pinned_layout.as_ref().and_then(|layout| {
                    (region == TableColumnRegion::Center && has_source_cells)
                        .then(|| layout.row_center_scroll_id(&render_key))
                });
                let mut region_children =
                    Vec::with_capacity(cells.len() + usize::from(uses_center_window) * 2);
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(leading_spacer_width));
                }
                let current_expansion_for_cells = current_expansion.clone();
                region_children.extend(cells.into_iter().map({
                    let table_id = table_id.clone();
                    let render_key = render_key.clone();
                    let row = row.clone();
                    let runtime = runtime.clone();
                    let focus_handle = focus_handle.clone();
                    let on_row_expansion_request = on_row_expansion_request.clone();
                    let on_cell_edit_change = on_cell_edit_change.clone();
                    let tree = tree.clone();
                    let tree_affordance_column_id = tree_affordance_column_id.clone();
                    move |cell| {
                        let tree_affordance = tree_affordance_column_id
                            .as_ref()
                            .is_some_and(|column_id| cell.column_id() == column_id);
                        render_table_body_cell(
                            table_id.clone(),
                            render_key.clone(),
                            metrics,
                            cell,
                            row.clone(),
                            tree.clone(),
                            tree_depth,
                            tree_branch,
                            tree_expanded,
                            tree_affordance,
                            runtime.clone(),
                            focus_handle.clone(),
                            current_expansion_for_cells.clone(),
                            on_row_expansion_request.clone(),
                            on_cell_edit_change.clone(),
                            measured_rows,
                        )
                        .into_any_element()
                    }
                }));
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(trailing_spacer_width));
                }

                let mut region_lane = div()
                    .min_w(px(0.0))
                    .flex()
                    .overflow_hidden()
                    .id(format!(
                        "table:{table_id}:row-region:{render_key}:{region_name}"
                    ))
                    .debug_selector({
                        let table_id = table_id.clone();
                        let render_key = render_key.clone();
                        let region_name = region_name.clone();
                        move || format!("table:{table_id}:row-region:{render_key}:{region_name}")
                    })
                    .w(gpui_px_from_ui(region_width))
                    .flex_none()
                    .children(region_children);

                region_lane = if measured_rows {
                    region_lane.items_start()
                } else {
                    region_lane.h_full().items_center()
                };

                let region_lane = region_lane.into_any_element();

                if let Some(center_scroll_id) = center_scroll_id {
                    div()
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
            },
        ))
        .when(!measured_rows, |this| {
            this.h(gpui_px_from_ui(row_for_layout.virtual_size()))
        })
        .into_any_element();
    row_element
}

fn table_row_region_cells_for_window(
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

fn render_table_body_cell(
    table_id: String,
    render_key: String,
    metrics: TableMetrics,
    cell: TableCellRenderPlan,
    row: TableRowRenderPlan,
    tree: Option<TableTreeRow>,
    tree_depth: usize,
    tree_branch: bool,
    tree_expanded: bool,
    tree_affordance: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
    measured_rows: bool,
) -> impl IntoElement {
    let column_id = cell.column_id().as_str().to_owned();
    let show_tree_affordance = tree_affordance && tree.is_some();
    let indent = ui_px(16.0) * tree_depth as f32;
    let mut content = Vec::new();
    if show_tree_affordance {
        content.push(
            div()
                .w(gpui_px_from_ui(indent))
                .when(!measured_rows, |this| this.h_full())
                .flex_none()
                .into_any_element(),
        );
        content.push(render_table_tree_toggle(
            table_id.clone(),
            render_key.clone(),
            row.clone(),
            tree_branch,
            tree_expanded,
            runtime,
            focus_handle,
            current_expansion,
            on_row_expansion_request,
        ));
    }
    let cell_value = cell.value().cloned();
    let cell_text = cell.text().to_owned();
    if let (Some(editor), Some(_)) = (cell.editor(), on_cell_edit_change.as_ref()) {
        let action = TableRowAction::from_render_plan(&row, TableInputModifiers::default());
        let column_id_for_change = cell.column_id().clone();
        let previous_value = cell_value.clone().unwrap_or_default();
        let select_options = cell
            .select_options()
            .iter()
            .map(|option| ListboxOption::new(option.value().to_owned(), option.label().to_owned()))
            .collect::<Vec<_>>();
        let selected_value = cell_value
            .as_ref()
            .map(TableCellValue::filter_text)
            .unwrap_or_default();
        let editor_id = format!("table:{table_id}:cell:{render_key}:{column_id}:editor");
        let editor_label = format!("Edit {column_id} for row {}", row.id().as_str());
        let editor_element = match editor {
            TableCellEditor::Text => {
                let on_change = on_cell_edit_change.clone();
                TextInput::new(editor_id, editor_label)
                    .value(cell_text)
                    .on_change(move |next_text, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    next_text,
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .with_size(metrics.size())
                    .into_any_element()
            }
            TableCellEditor::MultilineText { rows } => {
                let on_change = on_cell_edit_change.clone();
                Textarea::new(editor_id, editor_label)
                    .value(cell_text)
                    .rows(rows)
                    .on_change(move |next_text, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    next_text,
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .with_size(metrics.size())
                    .into_any_element()
            }
            TableCellEditor::Checkbox => {
                let on_change = on_cell_edit_change.clone();
                let checked = matches!(cell_value.as_ref(), Some(TableCellValue::Bool(true)));
                let editor_label = format!("Toggle {column_id} for row {}", row.id().as_str());
                Checkbox::new(editor_id)
                    .aria_label(editor_label)
                    .checked(checked)
                    .on_toggle(move |next_toggled, _, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    matches!(next_toggled, Toggled::True),
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .into_any_element()
            }
            TableCellEditor::Select => {
                let on_change = on_cell_edit_change.clone();
                Select::new(editor_id, editor_label)
                    .full_width(true)
                    .placeholder(cell_text.clone())
                    .selected(selected_value)
                    .options(select_options)
                    .on_select(move |selection, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    TableCellValue::Text(selection.value().to_owned()),
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .into_any_element()
            }
        };
        content.push(
            div()
                .id(format!(
                    "table:{table_id}:cell:{render_key}:{column_id}:editor-shell"
                ))
                .debug_selector({
                    let table_id = table_id.clone();
                    let render_key = render_key.clone();
                    let column_id = column_id.clone();
                    move || format!("table:{table_id}:cell:{render_key}:{column_id}:editor-shell")
                })
                .flex_1()
                .w_full()
                .min_w(px(0.0))
                .overflow_hidden()
                .block_mouse_except_scroll()
                .when(matches!(editor, TableCellEditor::Checkbox), |this| {
                    this.flex().justify_center().items_center()
                })
                .child(editor_element)
                .into_any_element(),
        );
    } else {
        content.push(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .when(measured_rows, |this| this.whitespace_normal())
                .when(!measured_rows, |this| this.truncate())
                .child(cell_text)
                .into_any_element(),
        );
    }

    let cell = div()
        .id(format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .debug_selector(move || format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .w(gpui_px_from_ui(cell.width()))
        .flex_none()
        .flex()
        .when(!measured_rows, |this| this.h_full().items_center())
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xe7e9e1))
        .text_xs()
        .text_color(rgb(0x2f3845))
        .ui_role(cell.role())
        .aria_column_index(cell.aria_column_index())
        .children(content)
        .when(measured_rows, |this| this.whitespace_normal())
        .when(!measured_rows, |this| this.truncate().whitespace_nowrap());

    cell.into_any_element()
}

fn render_table_tree_toggle(
    table_id: String,
    render_key: String,
    row: TableRowRenderPlan,
    tree_branch: bool,
    tree_expanded: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
) -> AnyElement {
    if !tree_branch {
        return div().w(px(18.0)).h(px(18.0)).flex_none().into_any_element();
    }

    let row_id = row.id().clone();
    let row_key = render_key.clone();
    let children_load_state = row
        .children_load_state()
        .cloned()
        .unwrap_or_else(TableRowChildrenLoadState::idle);
    let glyph = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => "...",
        TableRowChildrenLoadState::Failed { .. } => "!",
        TableRowChildrenLoadState::Idle if tree_expanded => "v",
        TableRowChildrenLoadState::Idle => ">",
    };
    let aria_label = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => {
            format!("Loading children for row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Failed { .. } => {
            format!("Retry loading row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle if tree_expanded => {
            format!("Collapse row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle => format!("Expand row {}", row.id().as_str()),
    };

    div()
        .id(format!("table:{table_id}:tree-toggle:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let row_key = row_key.clone();
            move || format!("table:{table_id}:tree-toggle:{row_key}")
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_role(Role::Button)
        .aria_label(aria_label)
        .aria_expanded(tree_expanded)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe8ede6)))
        .on_click(move |event: &ClickEvent, window, cx| {
            if !event.standard_click() || window.default_prevented() {
                return;
            }

            cx.stop_propagation();
            window.prevent_default();

            let next_expansion =
                toggle_table_expansion(current_expansion.clone(), row_id.clone(), !tree_expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row_id.clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(focus_handle) = focus_handle.as_ref() {
                focus_handle.focus(window, cx);
            }
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    &row,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, !tree_expanded),
                    window,
                    cx,
                );
            }
        })
        .child(glyph)
        .into_any_element()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRowKeyboardAction {
    Focus { index: usize, row_id: TableRowId },
    Toggle { expanded: bool },
    Activate,
}

fn table_row_keyboard_action(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    key: &str,
) -> Option<TableRowKeyboardAction> {
    let current_index = row.model_index();
    match key {
        "home" if !final_rows.is_empty() => Some(TableRowKeyboardAction::Focus {
            index: 0,
            row_id: final_rows[0].id().clone(),
        }),
        "end" if !final_rows.is_empty() => {
            let index = final_rows.len() - 1;
            Some(TableRowKeyboardAction::Focus {
                index,
                row_id: final_rows[index].id().clone(),
            })
        }
        "up" => current_index.checked_sub(1).and_then(|index| {
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }),
        "down" => {
            let index = current_index + 1;
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }
        "left" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(true) => {
            Some(TableRowKeyboardAction::Toggle { expanded: false })
        }
        "left" => row.row().parent_id().and_then(|parent_id| {
            final_rows
                .iter()
                .position(|candidate| candidate.id() == parent_id)
                .map(|index| TableRowKeyboardAction::Focus {
                    index,
                    row_id: parent_id.clone(),
                })
        }),
        "right" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(false) => {
            Some(TableRowKeyboardAction::Toggle { expanded: true })
        }
        "right" => final_rows
            .get(current_index + 1)
            .filter(|candidate| candidate.parent_id() == Some(row.id()))
            .map(|target| TableRowKeyboardAction::Focus {
                index: current_index + 1,
                row_id: target.id().clone(),
            }),
        "enter" | "space" => Some(TableRowKeyboardAction::Activate),
        _ => None,
    }
}

fn handle_table_row_key_down(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    vertical_scroll_handle: ScrollHandle,
    top_row_count: usize,
    center_total_row_count: usize,
    runtime: &Entity<TableRuntime>,
    current_expansion: TableExpansionState,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    let Some(action) = table_row_keyboard_action(row, final_rows, event.keystroke.key.as_str())
    else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();

    match action {
        TableRowKeyboardAction::Focus { index, row_id } => {
            let focus_handle = runtime.update(cx, |runtime, cx| runtime.set_focused(row_id, cx));
            if let Some(center_index) = index.checked_sub(top_row_count) {
                if center_index < center_total_row_count {
                    scroll_table_row_into_view(
                        &vertical_scroll_handle,
                        row.virtual_size(),
                        center_total_row_count,
                        center_index,
                    );
                }
            }
            if let Some(focus_handle) = focus_handle {
                focus_handle.focus(window, cx);
            }
            window.refresh();
        }
        TableRowKeyboardAction::Toggle { expanded } => {
            let next_expansion =
                toggle_table_expansion(current_expansion, row.id().clone(), expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, expanded),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
        TableRowKeyboardAction::Activate => {
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
            });
            if let Some(on_row_activate) = on_row_activate.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_activate(
                    TableRowActivation::new(action, TableRowActivationKind::Keyboard),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
    }
}

fn scroll_table_row_into_view(
    scroll_handle: &ScrollHandle,
    row_height: UiPx,
    row_count: usize,
    index: usize,
) {
    let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
    let row_height = nonnegative_px(row_height);
    if viewport_extent.as_f32() <= 0.0 || row_height.as_f32() <= 0.0 {
        return;
    }

    let total_extent = row_height * row_count as f32;
    let current_scroll_offset =
        UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
    let row_start = row_height * index as f32;
    let row_end = row_start + row_height;
    let max_scroll = nonnegative_px(total_extent - viewport_extent);
    let target = if row_start < current_scroll_offset {
        row_start
    } else if row_end > current_scroll_offset + viewport_extent {
        row_end - viewport_extent
    } else {
        current_scroll_offset
    };
    let target = target.max(UiPx::ZERO).min(max_scroll);

    scroll_handle.set_offset(point(px(0.0), -gpui_px_from_ui(target)));
}

fn render_table_lane_spacer(width: UiPx) -> AnyElement {
    div()
        .w(gpui_px_from_ui(width))
        .min_w(px(0.0))
        .flex_none()
        .h_full()
        .min_h(px(0.0))
        .into_any_element()
}

fn handle_table_vertical_scroll_wheel(
    scroll_handle: &ScrollHandle,
    event: &ScrollWheelEvent,
    window: &mut Window,
) {
    let delta = event.delta.pixel_delta(px(16.0));
    if delta.y.abs() <= delta.x.abs() {
        return;
    }

    let current = scroll_handle.offset();
    let max_offset_y = scroll_handle.max_offset().y;
    let next_y = (current.y + delta.y).clamp(-max_offset_y, px(0.0));

    if next_y != current.y {
        scroll_handle.set_offset(point(current.x, next_y));
        window.refresh();
    }
}

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::virtualization::measured_virtualizer_state;
    use super::*;
    use open_gpui_ui_core::{
        TableColumn, TableColumnPinning, TableRow, TableSort, VirtualizerRange,
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
