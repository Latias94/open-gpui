//! Table component backed by renderer-neutral row-model and virtualizer contracts.

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::scroll_area::ScrollArea;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
    rgb,
};
use open_gpui_ui_core::{
    Role, Sizable, Size, TableCellValue, TableColumn, TableColumnId, TableResolvedRow,
    TableResolvedState, TableSort, TableSortDirection, TableState, UiPx,
    VirtualizerItemMeasurement, VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerState,
    ui_px,
};
use std::collections::BTreeSet;
use std::rc::Rc;

/// Resolved table sizing and virtualization metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableMetrics {
    size: Size,
    header_height: UiPx,
    row_height: UiPx,
    cell_padding_x: UiPx,
    min_column_width: UiPx,
    viewport_extent: UiPx,
    overscan: usize,
}

impl TableMetrics {
    /// Resolves table metrics from the shared component size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            size,
            header_height: size.button_h(),
            row_height: size.list_row_h(),
            cell_padding_x: size.list_px(),
            min_column_width: match size {
                Size::XSmall => ui_px(96.0),
                Size::Small => ui_px(112.0),
                Size::Medium => ui_px(128.0),
                Size::Large => ui_px(144.0),
            },
            viewport_extent: match size {
                Size::XSmall => ui_px(160.0),
                Size::Small => ui_px(200.0),
                Size::Medium => ui_px(240.0),
                Size::Large => ui_px(280.0),
            },
            overscan: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 6,
                Size::Large => 8,
            },
        }
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns the fixed header row height.
    pub const fn header_height(self) -> UiPx {
        self.header_height
    }

    /// Returns the estimated body row height used by the virtualizer.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns horizontal cell padding.
    pub const fn cell_padding_x(self) -> UiPx {
        self.cell_padding_x
    }

    /// Returns the minimum visual column width.
    pub const fn min_column_width(self) -> UiPx {
        self.min_column_width
    }

    /// Returns the viewport extent used to resolve the virtual window.
    pub const fn viewport_extent(self) -> UiPx {
        self.viewport_extent
    }

    /// Returns the overscan row budget.
    pub const fn overscan(self) -> usize {
        self.overscan
    }
}

/// One resolved table column in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnRenderPlan {
    id: TableColumnId,
    label: String,
    aria_column_index: usize,
    sortable: bool,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
}

impl TableColumnRenderPlan {
    fn new(
        column: &TableColumn,
        aria_column_index: usize,
        sort_direction: Option<TableSortDirection>,
    ) -> Self {
        Self {
            id: column.id().clone(),
            label: column.label().to_owned(),
            aria_column_index,
            sortable: column.sortable(),
            sort_direction,
            sort_action: column
                .sortable()
                .then(|| TableHeaderAction::for_column(column, sort_direction)),
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns whether this column is sortable in the contract.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns the resolved sort direction for this column, when present.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the header action emitted when this sortable column is activated.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns the label exposed to assistive technology.
    pub fn accessible_label(&self) -> String {
        match self.sort_direction {
            Some(direction) => format!("{}, sorted {}", self.label, direction.as_str()),
            None => self.label.clone(),
        }
    }
}

/// Sort request emitted by an interactive table column header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableHeaderAction {
    column_id: TableColumnId,
    label: String,
    current_direction: Option<TableSortDirection>,
    next_direction: Option<TableSortDirection>,
    next_sorting: Vec<TableSort>,
}

impl TableHeaderAction {
    fn for_column(column: &TableColumn, current_direction: Option<TableSortDirection>) -> Self {
        let next_direction = match current_direction {
            None => Some(TableSortDirection::Ascending),
            Some(TableSortDirection::Ascending) => Some(TableSortDirection::Descending),
            Some(TableSortDirection::Descending) => None,
        };
        let next_sorting = next_direction
            .map(|direction| vec![TableSort::new(column.id().clone(), direction)])
            .unwrap_or_default();

        Self {
            column_id: column.id().clone(),
            label: column.label().to_owned(),
            current_direction,
            next_direction,
            next_sorting,
        }
    }

    /// Returns the activated column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the activated column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the currently resolved sort direction for the column.
    pub const fn current_direction(&self) -> Option<TableSortDirection> {
        self.current_direction
    }

    /// Returns the direction that should be applied by the next state update.
    pub const fn next_direction(&self) -> Option<TableSortDirection> {
        self.next_direction
    }

    /// Returns the next single-column sorting state.
    pub fn next_sorting(&self) -> &[TableSort] {
        &self.next_sorting
    }

    /// Applies this header action to a table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        state.with_sorting(self.next_sorting.clone())
    }
}

/// One resolved table cell in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellRenderPlan {
    column_id: TableColumnId,
    text: String,
    aria_column_index: usize,
    role: Role,
}

impl TableCellRenderPlan {
    fn new(column: &TableColumnRenderPlan, value: Option<&TableCellValue>) -> Self {
        Self {
            column_id: column.id().clone(),
            text: value.map(TableCellValue::filter_text).unwrap_or_default(),
            aria_column_index: column.aria_column_index(),
            role: Role::Cell,
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the display text resolved from the core cell value.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns the accessibility role for this cell.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// One resolved virtualized row to render.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowRenderPlan {
    row: TableResolvedRow,
    render_key: String,
    model_index: usize,
    aria_row_index: usize,
    measurement: VirtualizerItemMeasurement,
    cells: Vec<TableCellRenderPlan>,
    role: Role,
}

impl TableRowRenderPlan {
    fn new(
        row: TableResolvedRow,
        render_key: String,
        model_index: usize,
        measurement: VirtualizerItemMeasurement,
        columns: &[TableColumnRenderPlan],
    ) -> Self {
        let cells = columns
            .iter()
            .map(|column| TableCellRenderPlan::new(column, row.source().cell(column.id())))
            .collect();

        Self {
            row,
            render_key,
            model_index,
            aria_row_index: model_index + 2,
            measurement,
            cells,
            role: Role::Row,
        }
    }

    /// Returns the resolved core row.
    pub const fn row(&self) -> &TableResolvedRow {
        &self.row
    }

    /// Returns the stable row id.
    pub const fn id(&self) -> &open_gpui_ui_core::TableRowId {
        self.row.id()
    }

    /// Returns the unique render key used by element ids and virtualizer items.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns this row's index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.model_index
    }

    /// Returns the 1-based accessibility row index, including the header row.
    pub const fn aria_row_index(&self) -> usize {
        self.aria_row_index
    }

    /// Returns whether the row is selected by stable row id.
    pub const fn selected(&self) -> bool {
        self.row.selected()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Returns the cells in visible column order.
    pub fn cells(&self) -> &[TableCellRenderPlan] {
        &self.cells
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Fully resolved render contract for a concrete [`Table`] instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRenderPlan {
    table_id: String,
    label: String,
    metrics: TableMetrics,
    table: TableResolvedState,
    virtualizer: VirtualizerResolvedState,
    columns: Vec<TableColumnRenderPlan>,
    rows: Vec<TableRowRenderPlan>,
    role: Role,
    header_row_role: Role,
    column_header_role: Role,
    cell_role: Role,
}

impl TableRenderPlan {
    fn resolve(
        table_id: String,
        label: String,
        metrics: TableMetrics,
        table: TableResolvedState,
        virtualizer: VirtualizerResolvedState,
        columns: Vec<TableColumnRenderPlan>,
    ) -> Self {
        let source_rows = table.final_model().rows();
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let rows = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| {
                source_rows.get(measurement.index()).cloned().map(|row| {
                    let render_key = row_render_key(&row, &duplicate_row_ids);
                    TableRowRenderPlan::new(
                        row,
                        render_key,
                        measurement.index(),
                        measurement.clone(),
                        &columns,
                    )
                })
            })
            .collect();

        Self {
            table_id,
            label,
            metrics,
            table,
            virtualizer,
            columns,
            rows,
            role: Role::Table,
            header_row_role: Role::Row,
            column_header_role: Role::ColumnHeader,
            cell_role: Role::Cell,
        }
    }

    /// Returns the stable table id.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the accessible table label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns the resolved renderer-neutral table state.
    pub const fn table(&self) -> &TableResolvedState {
        &self.table
    }

    /// Returns the resolved renderer-neutral virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns visible columns in render order.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns virtualized rows in render order.
    pub fn rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns the accessibility role for the table root.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.header_row_role
    }

    /// Returns the accessibility role for header cells.
    pub const fn column_header_role(&self) -> Role {
        self.column_header_role
    }

    /// Returns the accessibility role for body cells.
    pub const fn cell_role(&self) -> Role {
        self.cell_role
    }

    /// Returns the accessibility row count, including the header row.
    pub fn aria_row_count(&self) -> usize {
        self.table.final_model().rows().len().saturating_add(1)
    }

    /// Returns the accessibility column count.
    pub fn aria_column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the number of body rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the visible body row count before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.virtualizer.visible_items().len()
    }
}

#[derive(Debug, Clone, Default)]
struct TableRuntime {
    scroll_handle: ScrollHandle,
}

/// A concrete GPUI table renderer using the Open GPUI row-model and virtualizer contracts.
#[derive(IntoElement)]
pub struct Table {
    id: String,
    label: SharedString,
    state: TableState,
    metrics: TableMetrics,
    snapshot: Option<VirtualizerSnapshot>,
    on_sort_requested: Option<Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>>,
}

impl Table {
    /// Creates a new table renderer from renderer-neutral table state.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>, state: TableState) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state,
            metrics: TableMetrics::from_size(Size::Medium),
            snapshot: None,
            on_sort_requested: None,
        }
    }

    /// Applies the accessible table label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics.overscan = overscan;
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics.row_height = nonnegative_px(row_height);
        self
    }

    /// Applies a fixed header height.
    pub fn header_height(mut self, header_height: UiPx) -> Self {
        self.metrics.header_height = nonnegative_px(header_height);
        self
    }

    /// Applies the fallback viewport extent used before layout metrics exist.
    pub fn viewport_extent(mut self, viewport_extent: UiPx) -> Self {
        self.metrics.viewport_extent = nonnegative_px(viewport_extent);
        self
    }

    /// Applies the minimum visual column width.
    pub fn min_column_width(mut self, min_column_width: UiPx) -> Self {
        self.metrics.min_column_width = nonnegative_px(min_column_width);
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

    /// Registers a handler for sortable column header activation.
    pub fn on_sort_requested(
        mut self,
        handler: impl Fn(TableHeaderAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort_requested = Some(Rc::new(handler));
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

    /// Resolves table row models and the virtual render window for a viewport.
    pub fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> TableRenderPlan {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let table = self.state.resolve();
        let columns = self.resolve_columns(&table);
        let final_rows = table.final_model().rows();
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let row_keys = final_rows
            .iter()
            .map(|row| row_render_key(row, &duplicate_row_ids));
        let mut virtualizer = VirtualizerState::new(final_rows.len(), metrics.row_height())
            .with_viewport_extent(metrics.viewport_extent())
            .with_overscan(metrics.overscan())
            .with_item_keys(row_keys);

        if let Some(snapshot) = self.snapshot.clone() {
            virtualizer = virtualizer.with_snapshot(snapshot);
        }
        virtualizer = virtualizer.with_scroll_offset(nonnegative_px(scroll_offset));

        TableRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            table,
            virtualizer.resolve(),
            columns,
        )
    }

    fn metrics_for_viewport(&self, viewport_extent: UiPx) -> TableMetrics {
        let mut metrics = self.metrics;
        let viewport_extent = nonnegative_px(viewport_extent);
        if viewport_extent.as_f32() > 0.0 {
            metrics.viewport_extent = viewport_extent;
        }
        metrics
    }

    fn resolve_columns(&self, table: &TableResolvedState) -> Vec<TableColumnRenderPlan> {
        table
            .visible_columns()
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let sort_direction = self
                    .state
                    .sorting()
                    .iter()
                    .find(|sort| sort.column() == column.id())
                    .map(|sort| sort.direction());
                TableColumnRenderPlan::new(column, index + 1, sort_direction)
            })
            .collect()
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
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableRuntime {
            scroll_handle: ScrollHandle::new(),
        });
        let scroll_handle = runtime.read(cx).scroll_handle.clone();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let scroll_offset = ui_px((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let plan = self.render_plan(scroll_offset, viewport_extent);
        let table_id = plan.table_id().to_owned();
        let label = plan.label().to_owned();
        let metrics = plan.metrics();
        let scroll_viewport_id = format!("table:{table_id}:body-scroll");
        let on_sort_requested = self.on_sort_requested.clone();

        div()
            .id(self.id)
            .debug_selector({
                let table_id = table_id.clone();
                move || format!("table:{table_id}:root")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
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
            .on_scroll_wheel(|_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .child(render_table_header(&plan, on_sort_requested))
            .child(
                div().flex_1().min_h(px(0.0)).overflow_hidden().child(
                    ScrollArea::new(scroll_viewport_id, render_table_body(&plan))
                        .vertical()
                        .scroll_handle(&scroll_handle)
                        .with_size(metrics.size()),
                ),
            )
    }
}

fn render_table_header(
    plan: &TableRenderPlan,
    on_sort_requested: Option<Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let columns = plan.columns().to_vec();

    div()
        .id(format!("table:{table_id}:header-row"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:header-row")
        })
        .h(gpui_px_from_ui(metrics.header_height()))
        .flex_none()
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf3f4ef))
        .ui_role(plan.row_role())
        .aria_row_index(1)
        .children(columns.into_iter().map(move |column| {
            let table_id = table_id.clone();
            let column_id = column.id().as_str().to_owned();
            let accessible_label = column.accessible_label();
            let sort_action = column.sort_action().cloned();
            let interactive_sort = sort_action.zip(on_sort_requested.clone());
            let sort_suffix = column
                .sort_direction()
                .map(|direction| match direction {
                    TableSortDirection::Ascending => " ↑",
                    TableSortDirection::Descending => " ↓",
                })
                .unwrap_or("");

            div()
                .id(format!("table:{table_id}:header:{column_id}"))
                .debug_selector(move || format!("table:{table_id}:header:{column_id}"))
                .min_w(gpui_px_from_ui(metrics.min_column_width()))
                .flex_1()
                .h_full()
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
                .ui_role(plan.column_header_role())
                .aria_label(accessible_label)
                .aria_column_index(column.aria_column_index())
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
                .child(format!("{}{}", column.label(), sort_suffix))
        }))
}

fn render_table_body(plan: &TableRenderPlan) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let total_size = plan.virtualizer().total_size();
    let rows = plan.rows().to_vec();

    div()
        .id(format!("table:{table_id}:body"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:body")
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(total_size))
        .children(rows.into_iter().map(move |row| {
            let table_id = table_id.clone();
            render_table_row(table_id, row, metrics)
        }))
}

fn render_table_row(
    table_id: String,
    row: TableRowRenderPlan,
    metrics: TableMetrics,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_background = if row.selected() {
        rgb(0xe7f0ff)
    } else if row.model_index().is_multiple_of(2) {
        rgb(0xffffff)
    } else {
        rgb(0xf8f9f3)
    };
    let cells = row.cells().to_vec();

    div()
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
        .h(gpui_px_from_ui(row.virtual_size()))
        .min_w(px(0.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .hover(|this| this.bg(rgb(0xeef2f7)))
        .ui_role(row.role())
        .aria_row_index(row.aria_row_index())
        .aria_selected(row.selected())
        .children(cells.into_iter().map(move |cell| {
            let table_id = table_id.clone();
            let render_key = render_key.clone();
            let column_id = cell.column_id().as_str().to_owned();

            div()
                .id(format!("table:{table_id}:cell:{render_key}:{column_id}"))
                .debug_selector(move || format!("table:{table_id}:cell:{render_key}:{column_id}"))
                .min_w(gpui_px_from_ui(metrics.min_column_width()))
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .px(gpui_px_from_ui(metrics.cell_padding_x()))
                .border_r_1()
                .border_color(rgb(0xe7e9e1))
                .truncate()
                .whitespace_nowrap()
                .text_xs()
                .text_color(rgb(0x2f3845))
                .ui_role(cell.role())
                .aria_column_index(cell.aria_column_index())
                .child(cell.text().to_owned())
        }))
}

fn row_render_key(
    row: &TableResolvedRow,
    duplicate_row_ids: &BTreeSet<open_gpui_ui_core::TableRowId>,
) -> String {
    if duplicate_row_ids.contains(row.id()) {
        format!("{}:{}", row.source_index(), row.id().as_str())
    } else {
        row.id().as_str().to_owned()
    }
}

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}
